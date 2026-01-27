use std::path::Path;
use std::process::Output;
use std::time::Duration;

use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::{Error, Result};

const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitService {
    UploadPack,
    ReceivePack,
}

impl GitService {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "git-upload-pack" => Some(Self::UploadPack),
            "git-receive-pack" => Some(Self::ReceivePack),
            _ => None,
        }
    }

    pub fn command_name(&self) -> &'static str {
        match self {
            Self::UploadPack => "git-upload-pack",
            Self::ReceivePack => "git-receive-pack",
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            Self::UploadPack => "application/x-git-upload-pack-result",
            Self::ReceivePack => "application/x-git-receive-pack-result",
        }
    }

    pub fn advertisement_content_type(&self) -> &'static str {
        match self {
            Self::UploadPack => "application/x-git-upload-pack-advertisement",
            Self::ReceivePack => "application/x-git-receive-pack-advertisement",
        }
    }

    pub fn is_write(&self) -> bool {
        matches!(self, Self::ReceivePack)
    }
}

pub async fn run_git_command(
    repo_path: &Path,
    service: GitService,
    advertise_refs: bool,
    input: Option<&[u8]>,
) -> Result<Output> {
    let mut cmd = Command::new(service.command_name());
    cmd.arg("--stateless-rpc");

    if advertise_refs {
        cmd.arg("--advertise-refs");
    }

    cmd.arg(repo_path);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(Error::Io)?;

    if let Some(data) = input {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(data).await.map_err(Error::Io)?;
        }
    }

    let output = tokio::time::timeout(GIT_COMMAND_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| Error::BadRequest("Git command timed out".into()))?
        .map_err(Error::Io)?;

    Ok(output)
}

pub async fn init_bare_repo(repo_path: &Path) -> Result<()> {
    if let Some(parent) = repo_path.parent() {
        fs::create_dir_all(parent).await.map_err(Error::Io)?;
    }

    let output = Command::new("git")
        .args(["init", "--bare"])
        .arg(repo_path)
        .output()
        .await
        .map_err(Error::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::BadRequest(format!(
            "Failed to init bare repo: {stderr}"
        )));
    }

    let head_path = repo_path.join("HEAD");
    fs::write(&head_path, "ref: refs/heads/main\n")
        .await
        .map_err(Error::Io)?;

    Ok(())
}

pub async fn calculate_repo_size(repo_path: &Path) -> Result<i64> {
    let mut total_size: i64 = 0;
    let mut entries = fs::read_dir(repo_path).await.map_err(Error::Io)?;

    while let Some(entry) = entries.next_entry().await.map_err(Error::Io)? {
        total_size += calculate_entry_size(&entry.path()).await?;
    }

    Ok(total_size)
}

fn calculate_entry_size(
    path: &Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<i64>> + Send + '_>> {
    Box::pin(async move {
        let metadata = fs::metadata(path).await.map_err(Error::Io)?;

        if metadata.is_file() {
            return Ok(metadata.len() as i64);
        }

        if metadata.is_dir() {
            let mut total: i64 = 0;
            let mut entries = fs::read_dir(path).await.map_err(Error::Io)?;

            while let Some(entry) = entries.next_entry().await.map_err(Error::Io)? {
                total += calculate_entry_size(&entry.path()).await?;
            }

            return Ok(total);
        }

        Ok(0)
    })
}

pub fn format_pkt_line_header(service: GitService) -> Vec<u8> {
    let service_name = service.command_name();
    let service_line = format!("# service={service_name}\n");
    let length = service_line.len() + 4;
    let mut result = format!("{length:04x}{service_line}").into_bytes();
    result.extend_from_slice(b"0000");
    result
}

#[must_use]
pub fn repo_path(data_dir: &Path, namespace_id: &str, repo_name: &str) -> std::path::PathBuf {
    data_dir
        .join("repos")
        .join(namespace_id)
        .join(format!("{repo_name}.git"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_service_from_str() {
        assert_eq!(
            GitService::from_str("git-upload-pack"),
            Some(GitService::UploadPack)
        );
        assert_eq!(
            GitService::from_str("git-receive-pack"),
            Some(GitService::ReceivePack)
        );
        assert_eq!(GitService::from_str("invalid"), None);
    }

    #[test]
    fn test_format_pkt_line_header() {
        let header = format_pkt_line_header(GitService::UploadPack);
        let header_str = String::from_utf8_lossy(&header);
        assert!(header_str.starts_with("001e# service=git-upload-pack\n"));
        assert!(header_str.ends_with("0000"));
    }

    #[test]
    fn test_repo_path() {
        let path = repo_path(Path::new("/data"), "ns123", "myrepo");
        assert_eq!(path, Path::new("/data/repos/ns123/myrepo.git"));
    }
}
