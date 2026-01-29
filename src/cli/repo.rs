use std::process::Command;

use inquire::{MultiSelect, Select};
use serde::Serialize;

use super::credentials::load_credentials;
use super::http_client::{ApiClient, NamespaceMap, PaginatedResponse};
use super::pickers::{TagDisplay, confirm_action, repos_to_displays};
use crate::types::{Folder, Repo, Tag};

/// Parses a repo reference in the format "namespace/name" or "name".
/// Returns (Some(namespace), name) if namespace was explicit, (None, name) otherwise.
/// Validates that the reference doesn't contain extra slashes.
pub fn parse_repo_ref(repo_ref: &str) -> anyhow::Result<(Option<String>, String)> {
    let slash_count = repo_ref.chars().filter(|c| *c == '/').count();
    match slash_count {
        0 => Ok((None, repo_ref.to_string())),
        1 => {
            let (ns, name) = repo_ref.split_once('/').unwrap();
            if ns.is_empty() || name.is_empty() {
                anyhow::bail!(
                    "Invalid repo reference '{}': namespace and name cannot be empty",
                    repo_ref
                );
            }
            Ok((Some(ns.to_string()), name.to_string()))
        }
        _ => anyhow::bail!(
            "Invalid repo reference '{}': expected 'namespace/repo' or 'repo'",
            repo_ref
        ),
    }
}

/// Resolves the namespace name, fetching the primary namespace if not provided.
pub fn resolve_namespace_name(
    namespace: Option<String>,
    client: &ApiClient,
) -> anyhow::Result<String> {
    if let Some(ns) = namespace {
        return Ok(ns);
    }
    let namespaces = client.fetch_namespaces()?;
    namespaces
        .into_iter()
        .find(|n| n.is_primary)
        .map(|n| n.namespace.name)
        .ok_or_else(|| anyhow::anyhow!("No namespace available"))
}

fn find_repo_by_ref(
    repos: Vec<Repo>,
    namespace_name: &str,
    repo_name: &str,
    namespace_map: &NamespaceMap,
) -> Option<Repo> {
    repos.into_iter().find(|r| {
        let ns = namespace_map
            .get(&r.namespace_id)
            .map(|s| s.as_str())
            .unwrap_or("");
        ns == namespace_name && r.name == repo_name
    })
}

fn select_repo(
    repos: Vec<Repo>,
    repo_ref: Option<&str>,
    namespace_map: &NamespaceMap,
    client: &ApiClient,
    non_interactive: bool,
    prompt: &str,
) -> anyhow::Result<Option<Repo>> {
    if let Some(repo_str) = repo_ref {
        let (ns, repo_name) = parse_repo_ref(repo_str)?;
        let ns_name = resolve_namespace_name(ns, client)?;
        let repo = find_repo_by_ref(repos, &ns_name, &repo_name, namespace_map)
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", repo_str))?;
        return Ok(Some(repo));
    }

    if non_interactive {
        anyhow::bail!("Repository argument is required in non-interactive mode");
    }

    let displays = repos_to_displays(repos, namespace_map);
    if displays.is_empty() {
        println!("No repositories found.");
        return Ok(None);
    }

    let selected = Select::new(prompt, displays)
        .with_page_size(15)
        .with_vim_mode(true)
        .with_help_message("Type to filter, Enter to select")
        .prompt()?;

    Ok(Some(selected.repo))
}

fn get_namespace_name(repo: &Repo, namespace_map: &NamespaceMap) -> String {
    namespace_map
        .get(&repo.namespace_id)
        .cloned()
        .unwrap_or_else(|| "?".to_string())
}

#[derive(Serialize)]
struct RepoTagsRequest {
    tag_ids: Vec<String>,
}

pub fn run_repo_delete(
    repo_ref: Option<String>,
    non_interactive: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let resp: PaginatedResponse<Repo> = client.get_raw("/repos")?;
    let namespace_map = client.fetch_namespace_map()?;

    let repo = match select_repo(
        resp.data,
        repo_ref.as_deref(),
        &namespace_map,
        &client,
        non_interactive,
        "Select repository to delete:",
    )? {
        Some(r) => r,
        None => return Ok(()),
    };

    let ns_name = get_namespace_name(&repo, &namespace_map);

    let confirmed = confirm_action(
        &format!("Delete repository '{}/{}'?", ns_name, repo.name),
        yes,
        non_interactive,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    client.delete(&format!("/repos/{}", repo.id))?;

    println!();
    println!("Deleted repository '{}/{}'", ns_name, repo.name);
    println!();

    Ok(())
}

pub fn run_repo_clone(repo_ref: Option<String>, non_interactive: bool) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let resp: PaginatedResponse<Repo> = client.get_raw("/repos")?;
    let namespace_map = client.fetch_namespace_map()?;

    let repo = match select_repo(
        resp.data,
        repo_ref.as_deref(),
        &namespace_map,
        &client,
        non_interactive,
        "Select repository to clone:",
    )? {
        Some(r) => r,
        None => return Ok(()),
    };

    let ns_name = get_namespace_name(&repo, &namespace_map);
    let clone_url = format!("{}/git/{}/{}.git", client.base_url(), ns_name, repo.name);

    println!("Cloning {}/{}...", ns_name, repo.name);

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
    println!("Cloned to ./{}", repo.name);
    println!();

    Ok(())
}

pub fn run_repo_tag(
    repo_ref: Option<String>,
    tags: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let repos_resp: PaginatedResponse<Repo> = client.get_raw("/repos")?;
    let namespace_map = client.fetch_namespace_map()?;

    let repo = match select_repo(
        repos_resp.data,
        repo_ref.as_deref(),
        &namespace_map,
        &client,
        non_interactive,
        "Select repository:",
    )? {
        Some(r) => r,
        None => return Ok(()),
    };

    let ns_name = get_namespace_name(&repo, &namespace_map);
    let tags_resp: PaginatedResponse<Tag> =
        client.get_raw(&format!("/tags?namespace={}", ns_name))?;

    let current_tags: Vec<Tag> = client.get(&format!("/repos/{}/tags", repo.id))?;
    let current_tag_ids: std::collections::HashSet<_> =
        current_tags.iter().map(|t| &t.id).collect();

    let selected_tag_ids = if let Some(tag_str) = tags {
        tag_str.split(',').map(|s| s.trim().to_string()).collect()
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
    let _: Vec<Tag> = client.put(&format!("/repos/{}/tags", repo.id), &request)?;

    println!();
    println!("Updated tags on '{}/{}'", ns_name, repo.name);
    println!();

    Ok(())
}

#[derive(Serialize)]
struct SetRepoFolderRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    folder_path: Option<String>,
}

pub fn run_repo_move(
    repo_ref: Option<String>,
    folder_path: String,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let repos_resp: PaginatedResponse<Repo> = client.get_raw("/repos")?;
    let namespace_map = client.fetch_namespace_map()?;

    let repo = match select_repo(
        repos_resp.data,
        repo_ref.as_deref(),
        &namespace_map,
        &client,
        non_interactive,
        "Select repository to move:",
    )? {
        Some(r) => r,
        None => return Ok(()),
    };

    let ns_name = get_namespace_name(&repo, &namespace_map);

    // Convert empty string to None (move to root)
    let folder_path_opt = if folder_path.is_empty() {
        None
    } else {
        Some(folder_path)
    };

    let request = SetRepoFolderRequest {
        folder_path: folder_path_opt.clone(),
    };

    let folders: Vec<Folder> = client.post(&format!("/repos/{}/folders", repo.id), &request)?;

    println!();
    if let Some(folder) = folders.first() {
        println!(
            "Moved '{}/{}' to folder '{}'",
            ns_name, repo.name, folder.path
        );
    } else {
        println!("Moved '{}/{}' to root", ns_name, repo.name);
    }
    println!();

    Ok(())
}
