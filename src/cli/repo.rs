use std::process::Command;

use inquire::{MultiSelect, Select};
use serde::Serialize;

use super::credentials::load_credentials;
use super::http_client::{ApiClient, PaginatedResponse};
use super::pickers::{confirm_action, print_tags_list, repos_to_displays, TagDisplay};
use crate::types::{Repo, Tag};

#[derive(Serialize)]
struct RepoListOutput {
    id: String,
    namespace: String,
    name: String,
    created_at: String,
}

#[derive(Serialize)]
struct RepoTagsRequest {
    tag_ids: Vec<String>,
}

pub fn run_repo_delete(
    repo_id: Option<String>,
    namespace: Option<String>,
    list: bool,
    non_interactive: bool,
    json: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let path = match &namespace {
        Some(ns) => format!("/repos?namespace={}", ns),
        None => "/repos".to_string(),
    };
    let resp: PaginatedResponse<Repo> = client.get_raw(&path)?;

    let namespace_map = client.fetch_namespace_map()?;

    if list {
        if json {
            let output: Vec<RepoListOutput> = resp
                .data
                .iter()
                .map(|r| RepoListOutput {
                    id: r.id.clone(),
                    namespace: namespace_map
                        .get(&r.namespace_id)
                        .cloned()
                        .unwrap_or_default(),
                    name: r.name.clone(),
                    created_at: r.created_at.to_rfc3339(),
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else if resp.data.is_empty() {
            println!("No repositories found.");
        } else {
            println!();
            for repo in &resp.data {
                let ns_name = namespace_map
                    .get(&repo.namespace_id)
                    .map(|s| s.as_str())
                    .unwrap_or("?");
                println!("  {}/{}", ns_name, repo.name);
            }
            println!();
        }
        return Ok(());
    }

    let repo = if let Some(id) = repo_id {
        resp.data
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", id))?
    } else if non_interactive {
        anyhow::bail!("--repo-id is required in non-interactive mode");
    } else {
        let displays = repos_to_displays(resp.data, &namespace_map);

        if displays.is_empty() {
            println!("No repositories found.");
            return Ok(());
        }

        let selected = Select::new("Select repository to delete:", displays)
            .with_page_size(15)
            .with_vim_mode(true)
            .with_help_message("Type to filter, Enter to select")
            .prompt()?;

        selected.repo
    };

    let repo_name = &repo.name;
    let ns_name = namespace_map
        .get(&repo.namespace_id)
        .map(|s| s.as_str())
        .unwrap_or("?");

    let confirmed = confirm_action(
        &format!("Delete repository '{}/{}'?", ns_name, repo_name),
        yes,
        non_interactive,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    client.delete(&format!("/repos/{}", repo.id))?;

    println!();
    println!("Deleted repository '{}/{}'", ns_name, repo_name);
    println!();

    Ok(())
}

pub fn run_repo_clone(
    repo: Option<String>,
    list: bool,
    non_interactive: bool,
    json: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let resp: PaginatedResponse<Repo> = client.get_raw("/repos")?;

    let namespace_map = client.fetch_namespace_map()?;

    if list {
        if json {
            let output: Vec<RepoListOutput> = resp
                .data
                .iter()
                .map(|r| RepoListOutput {
                    id: r.id.clone(),
                    namespace: namespace_map
                        .get(&r.namespace_id)
                        .cloned()
                        .unwrap_or_default(),
                    name: r.name.clone(),
                    created_at: r.created_at.to_rfc3339(),
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else if resp.data.is_empty() {
            println!("No repositories found.");
        } else {
            println!();
            for r in &resp.data {
                let ns_name = namespace_map
                    .get(&r.namespace_id)
                    .map(|s| s.as_str())
                    .unwrap_or("?");
                println!("  {}/{}", ns_name, r.name);
            }
            println!();
        }
        return Ok(());
    }

    let selected_repo = if let Some(ref repo_ref) = repo {
        if repo_ref.contains('/') {
            let parts: Vec<&str> = repo_ref.splitn(2, '/').collect();
            let ns = parts[0];
            let name = parts[1];
            resp.data
                .into_iter()
                .find(|r| {
                    let ns_name = namespace_map
                        .get(&r.namespace_id)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    ns_name == ns && r.name == name
                })
                .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_ref))?
        } else {
            resp.data
                .into_iter()
                .find(|r| r.id == *repo_ref)
                .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_ref))?
        }
    } else if non_interactive {
        anyhow::bail!("--repo is required in non-interactive mode");
    } else {
        let displays = repos_to_displays(resp.data, &namespace_map);

        if displays.is_empty() {
            println!("No repositories found.");
            return Ok(());
        }

        let selected = Select::new("Select repository to clone:", displays)
            .with_page_size(15)
            .with_vim_mode(true)
            .with_help_message("Type to filter, Enter to select")
            .prompt()?;

        selected.repo
    };

    let ns_name = namespace_map
        .get(&selected_repo.namespace_id)
        .map(|s| s.as_str())
        .unwrap_or("?");
    let clone_url = format!(
        "{}/git/{}/{}.git",
        client.base_url(),
        ns_name,
        selected_repo.name
    );

    println!("Cloning {}/{}...", ns_name, selected_repo.name);

    let auth_header = format!("http.extraHeader=Authorization: Bearer {}", creds.token);
    let status = Command::new("git")
        .arg("-c")
        .arg(&auth_header)
        .args(["clone", &clone_url])
        .status()?;

    if !status.success() {
        anyhow::bail!("Git clone failed. Check the output above for details.");
    }

    println!();
    println!("Cloned to ./{}", selected_repo.name);
    println!();

    Ok(())
}

pub fn run_repo_tag(
    repo_id: Option<String>,
    namespace: Option<String>,
    tags: Option<String>,
    list: bool,
    non_interactive: bool,
    json: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    if list {
        let path = match &namespace {
            Some(ns) => format!("/tags?namespace={}", ns),
            None => "/tags".to_string(),
        };
        let resp: PaginatedResponse<Tag> = client.get_raw(&path)?;

        if json {
            println!("{}", serde_json::to_string_pretty(&resp.data)?);
        } else {
            print_tags_list(&resp.data);
        }
        return Ok(());
    }

    let repos_path = match &namespace {
        Some(ns) => format!("/repos?namespace={}", ns),
        None => "/repos".to_string(),
    };
    let repos_resp: PaginatedResponse<Repo> = client.get_raw(&repos_path)?;

    let namespace_map = client.fetch_namespace_map()?;

    let selected_repo = if let Some(id) = repo_id {
        repos_resp
            .data
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", id))?
    } else if non_interactive {
        anyhow::bail!("--repo-id is required in non-interactive mode");
    } else {
        let displays = repos_to_displays(repos_resp.data, &namespace_map);

        if displays.is_empty() {
            println!("No repositories found.");
            return Ok(());
        }

        let selected = Select::new("Select repository:", displays)
            .with_page_size(15)
            .with_vim_mode(true)
            .with_help_message("Type to filter, Enter to select")
            .prompt()?;

        selected.repo
    };

    let ns_name = namespace_map
        .get(&selected_repo.namespace_id)
        .cloned()
        .unwrap_or_default();
    let tags_resp: PaginatedResponse<Tag> = client.get_raw(&format!("/tags?namespace={}", ns_name))?;

    let current_tags: Vec<Tag> = client.get(&format!("/repos/{}/tags", selected_repo.id))?;
    let current_tag_ids: std::collections::HashSet<_> =
        current_tags.iter().map(|t| &t.id).collect();

    let selected_tag_ids = if let Some(tag_str) = tags {
        tag_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect()
    } else if non_interactive {
        anyhow::bail!("--tags is required in non-interactive mode");
    } else {
        if tags_resp.data.is_empty() {
            println!("No tags available. Create tags first with 'cutman tag create'.");
            return Ok(());
        }

        let options: Vec<String> = tags_resp
            .data
            .iter()
            .map(|t| TagDisplay { tag: t.clone() }.to_string())
            .collect();

        let defaults: Vec<usize> = tags_resp
            .data
            .iter()
            .enumerate()
            .filter(|(_, t)| current_tag_ids.contains(&t.id))
            .map(|(i, _)| i)
            .collect();

        let selected = MultiSelect::new("Select tags:", options)
            .with_default(&defaults)
            .with_page_size(10)
            .with_vim_mode(true)
            .with_help_message("Space to toggle, Enter to confirm")
            .prompt()?;

        selected
            .iter()
            .filter_map(|name| {
                let name_only = name.split(' ').next().unwrap_or(name);
                tags_resp
                    .data
                    .iter()
                    .find(|t| t.name == name_only)
                    .map(|t| t.id.clone())
            })
            .collect()
    };

    let request = RepoTagsRequest {
        tag_ids: selected_tag_ids,
    };
    let _: Vec<Tag> = client.put(&format!("/repos/{}/tags", selected_repo.id), &request)?;

    println!();
    println!("Updated tags on '{}/{}'", ns_name, selected_repo.name);
    println!();

    Ok(())
}
