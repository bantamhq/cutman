use inquire::{Select, Text};
use serde::Serialize;

use super::credentials::load_credentials;
use super::http_client::ApiClient;
use super::pickers::confirm_action;
use super::repo::resolve_namespace_name;
use crate::types::Folder;

#[derive(Debug, Serialize)]
struct CreateFolderRequest {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: Option<String>,
}

#[derive(Debug, Serialize)]
struct UpdateFolderRequest {
    path: String,
}

fn normalize_user_path(path: &str) -> String {
    format!("/{}", path.trim_start_matches('/'))
}

fn folder_matches_path(folder: &Folder, path: &str) -> bool {
    folder.path == path || folder.path == normalize_user_path(path)
}

pub fn run_folder_create(
    path: Option<String>,
    namespace: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let ns_name = resolve_namespace_name(namespace.clone(), &client)?;

    let folder_path = if let Some(p) = path {
        p
    } else if non_interactive {
        anyhow::bail!("Path argument is required in non-interactive mode");
    } else {
        Text::new("Folder path:")
            .with_help_message("e.g., /engineering/backend")
            .prompt()?
    };

    let request = CreateFolderRequest {
        path: folder_path.clone(),
        namespace: Some(ns_name.clone()),
    };

    let folder: Folder = client.post("/folders", &request)?;

    println!();
    println!("Created folder '{}'", folder.path);
    if let Some(parent) = folder.parent_path() {
        println!("  Parent: {}", parent);
    }
    println!("  Namespace: {}", ns_name);
    println!();

    Ok(())
}

pub fn run_folder_list(namespace: Option<String>, _non_interactive: bool) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let ns_name = resolve_namespace_name(namespace, &client)?;

    let folders: Vec<Folder> = client.get(&format!("/folders?namespace={}", ns_name))?;

    if folders.is_empty() {
        println!("No folders found in namespace '{}'.", ns_name);
        return Ok(());
    }

    println!();
    println!("Folders in '{}':", ns_name);
    println!();

    for folder in &folders {
        println!("  {}", folder.path);
    }

    println!();
    println!("{} folder(s) total", folders.len());
    println!();

    Ok(())
}

pub fn run_folder_delete(
    path: Option<String>,
    namespace: Option<String>,
    non_interactive: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let ns_name = resolve_namespace_name(namespace, &client)?;

    let folders: Vec<Folder> = client.get(&format!("/folders?namespace={}", ns_name))?;

    if folders.is_empty() {
        println!("No folders found.");
        return Ok(());
    }

    let folder = if let Some(p) = path {
        folders
            .into_iter()
            .find(|f| folder_matches_path(f, &p))
            .ok_or_else(|| anyhow::anyhow!("Folder not found: {}", p))?
    } else if non_interactive {
        anyhow::bail!("Path argument is required in non-interactive mode");
    } else {
        let options: Vec<String> = folders.iter().map(|f| f.path.clone()).collect();
        let selected = Select::new("Select folder to delete:", options)
            .with_page_size(15)
            .with_vim_mode(true)
            .prompt()?;

        folders
            .into_iter()
            .find(|f| f.path == selected)
            .ok_or_else(|| anyhow::anyhow!("Folder not found"))?
    };

    let confirmed = confirm_action(
        &format!(
            "Delete folder '{}' and all child folders? Repos will be moved to root.",
            folder.path
        ),
        yes,
        non_interactive,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    client.delete(&format!("/folders/{}", folder.id))?;

    println!();
    println!("Deleted folder '{}'", folder.path);
    println!();

    Ok(())
}

pub fn run_folder_move(
    old_path: Option<String>,
    new_path: Option<String>,
    namespace: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let ns_name = resolve_namespace_name(namespace, &client)?;

    let folders: Vec<Folder> = client.get(&format!("/folders?namespace={}", ns_name))?;

    if folders.is_empty() {
        println!("No folders found.");
        return Ok(());
    }

    let folder = if let Some(p) = old_path {
        folders
            .into_iter()
            .find(|f| folder_matches_path(f, &p))
            .ok_or_else(|| anyhow::anyhow!("Folder not found: {}", p))?
    } else if non_interactive {
        anyhow::bail!("Old path argument is required in non-interactive mode");
    } else {
        let options: Vec<String> = folders.iter().map(|f| f.path.clone()).collect();
        let selected = Select::new("Select folder to move:", options)
            .with_page_size(15)
            .with_vim_mode(true)
            .prompt()?;

        folders
            .into_iter()
            .find(|f| f.path == selected)
            .ok_or_else(|| anyhow::anyhow!("Folder not found"))?
    };

    let dest_path = if let Some(p) = new_path {
        p
    } else if non_interactive {
        anyhow::bail!("New path argument is required in non-interactive mode");
    } else {
        Text::new("New path:")
            .with_initial_value(&folder.path)
            .with_help_message("Enter the new path for this folder")
            .prompt()?
    };

    let request = UpdateFolderRequest {
        path: dest_path.clone(),
    };
    let updated: Folder = client.patch(&format!("/folders/{}", folder.id), &request)?;

    println!();
    println!("Moved folder '{}' to '{}'", folder.path, updated.path);
    println!();

    Ok(())
}
