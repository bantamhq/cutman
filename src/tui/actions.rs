use serde::Serialize;

use crate::cli::http_client::{ApiClient, NamespaceWithPrimary, PaginatedResponse};
use crate::types::{Folder, Repo};

pub fn fetch_namespaces(client: &ApiClient) -> anyhow::Result<Vec<NamespaceWithPrimary>> {
    client.fetch_namespaces()
}

pub fn fetch_root_folders(client: &ApiClient, namespace: &str) -> anyhow::Result<Vec<Folder>> {
    fetch_folders_with_parent(client, namespace, None)
}

pub fn fetch_folder_children(client: &ApiClient, namespace: &str, parent_id: &str) -> anyhow::Result<Vec<Folder>> {
    fetch_folders_with_parent(client, namespace, Some(parent_id))
}

fn fetch_folders_with_parent(
    client: &ApiClient,
    namespace: &str,
    parent_id: Option<&str>,
) -> anyhow::Result<Vec<Folder>> {
    let mut folders = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let path = match (&cursor, parent_id) {
            (Some(c), Some(pid)) => {
                format!("/folders?namespace={}&parent_id={}&cursor={}", namespace, pid, c)
            }
            (None, Some(pid)) => format!("/folders?namespace={}&parent_id={}", namespace, pid),
            (Some(c), None) => format!("/folders?namespace={}&cursor={}", namespace, c),
            (None, None) => format!("/folders?namespace={}", namespace),
        };

        let response: PaginatedResponse<Folder> = client.get_raw(&path)?;
        folders.extend(response.data);

        if response.has_more {
            cursor = response.next_cursor;
        } else {
            break;
        }
    }

    Ok(folders)
}

pub fn fetch_repos(client: &ApiClient, namespace: &str) -> anyhow::Result<Vec<Repo>> {
    let mut all_repos = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let path = match &cursor {
            Some(c) => format!("/repos?namespace={}&cursor={}", namespace, c),
            None => format!("/repos?namespace={}", namespace),
        };

        let response: PaginatedResponse<Repo> = client.get_raw(&path)?;
        all_repos.extend(response.data);

        if response.has_more {
            cursor = response.next_cursor;
        } else {
            break;
        }
    }

    Ok(all_repos)
}

#[derive(Debug, Serialize)]
pub struct CreateFolderRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

pub fn create_folder(
    client: &ApiClient,
    name: &str,
    parent_id: Option<&str>,
    namespace: Option<&str>,
) -> anyhow::Result<Folder> {
    let request = CreateFolderRequest {
        name: name.to_string(),
        parent_id: parent_id.map(String::from),
        namespace: namespace.map(String::from),
    };
    client.post("/folders", &request)
}

#[derive(Debug, Serialize)]
pub struct UpdateFolderRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

pub fn rename_folder(
    client: &ApiClient,
    folder_id: &str,
    new_name: &str,
) -> anyhow::Result<Folder> {
    let request = UpdateFolderRequest {
        name: Some(new_name.to_string()),
        parent_id: None,
    };
    client.patch(&format!("/folders/{}", folder_id), &request)
}

pub fn move_folder(
    client: &ApiClient,
    folder_id: &str,
    new_parent_id: Option<&str>,
) -> anyhow::Result<Folder> {
    let request = UpdateFolderRequest {
        name: None,
        parent_id: new_parent_id.map(String::from),
    };
    client.patch(&format!("/folders/{}", folder_id), &request)
}

pub fn delete_folder(client: &ApiClient, folder_id: &str, recursive: bool) -> anyhow::Result<()> {
    let path = if recursive {
        format!("/folders/{}?recursive=true&force=true", folder_id)
    } else {
        format!("/folders/{}?force=true", folder_id)
    };
    client.delete(&path)
}

#[derive(Debug, Serialize)]
pub struct SetRepoFolderRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

pub fn move_repo(
    client: &ApiClient,
    repo_id: &str,
    folder_id: Option<&str>,
) -> anyhow::Result<()> {
    let request = SetRepoFolderRequest {
        folder_id: folder_id.map(String::from),
    };
    let _response: Vec<Folder> = client.post(&format!("/repos/{}/folders", repo_id), &request)?;
    Ok(())
}
