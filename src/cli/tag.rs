use inquire::{Select, Text};
use serde::Serialize;

use super::credentials::load_credentials;
use super::http_client::{ApiClient, PaginatedResponse};
use super::pickers::{TagDisplay, confirm_action};
use crate::types::Tag;

#[derive(Serialize)]
struct CreateTagRequest {
    name: String,
    color: Option<String>,
    namespace: Option<String>,
}

pub fn run_tag_create(
    name: Option<String>,
    color: Option<String>,
    namespace: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let name = if let Some(n) = name {
        n
    } else if non_interactive {
        anyhow::bail!("--name is required in non-interactive mode");
    } else {
        Text::new("Tag name:").prompt()?
    };

    let color = if color.is_some() {
        color
    } else if non_interactive {
        None
    } else {
        let input = Text::new("Tag color (hex, optional):")
            .with_placeholder("e.g., ff0000")
            .prompt()?;
        if input.is_empty() { None } else { Some(input) }
    };

    let request = CreateTagRequest {
        name: name.clone(),
        color,
        namespace,
    };

    let tag: Tag = client.post("/tags", &request)?;

    println!();
    println!("Created tag '{}'", tag.name);
    println!();

    Ok(())
}

pub fn run_tag_delete(
    tag_id: Option<String>,
    namespace: Option<String>,
    non_interactive: bool,
    yes: bool,
    force: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let path = match &namespace {
        Some(ns) => format!("/tags?namespace={}", ns),
        None => "/tags".to_string(),
    };
    let resp: PaginatedResponse<Tag> = client.get_raw(&path)?;

    let tag = if let Some(id) = tag_id {
        resp.data
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("Tag not found: {}", id))?
    } else if non_interactive {
        anyhow::bail!("--tag-id is required in non-interactive mode");
    } else {
        let displays: Vec<TagDisplay> = resp
            .data
            .into_iter()
            .map(|tag| TagDisplay { tag })
            .collect();

        if displays.is_empty() {
            println!("No tags found.");
            return Ok(());
        }

        let selected = Select::new("Select tag to delete:", displays)
            .with_page_size(15)
            .with_vim_mode(true)
            .with_help_message("Type to filter, Enter to select")
            .prompt()?;

        selected.tag
    };

    let confirmed = confirm_action(&format!("Delete tag '{}'?", tag.name), yes, non_interactive)?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    let delete_path = if force {
        format!("/tags/{}?force=true", tag.id)
    } else {
        format!("/tags/{}", tag.id)
    };

    client.delete(&delete_path)?;

    println!();
    println!("Deleted tag '{}'", tag.name);
    println!();

    Ok(())
}
