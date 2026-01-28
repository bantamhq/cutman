use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub server_url: String,
    pub token: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CredentialsFile {
    pub default: Option<Credentials>,
}

pub fn credentials_path() -> anyhow::Result<PathBuf> {
    let dirs = ProjectDirs::from("", "", "cutman")
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory. Is $HOME set?"))?;
    Ok(dirs.config_dir().join("credentials.toml"))
}

pub fn load_credentials() -> anyhow::Result<Credentials> {
    let path = credentials_path()?;
    let content = fs::read_to_string(&path)
        .map_err(|_| anyhow::anyhow!("Not logged in. Run 'cutman auth login' first."))?;
    let file: CredentialsFile = toml::from_str(&content)?;
    file.default
        .ok_or_else(|| anyhow::anyhow!("Credentials file is corrupted. Run 'cutman auth login' to fix."))
}

pub fn save_credentials(creds: &Credentials) -> anyhow::Result<()> {
    let path = credentials_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = CredentialsFile {
        default: Some(creds.clone()),
    };
    let content = toml::to_string_pretty(&file)?;
    fs::write(&path, content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}
