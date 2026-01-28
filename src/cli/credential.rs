use std::io::{self, BufRead, Write};

use super::credentials::{credentials_path, delete_credentials, load_credentials};

/// Parse key=value pairs from stdin (Git credential protocol)
fn parse_credential_input() -> io::Result<std::collections::HashMap<String, String>> {
    let stdin = io::stdin();
    let mut attrs = std::collections::HashMap::new();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once('=') {
            attrs.insert(key.to_string(), value.to_string());
        }
    }

    Ok(attrs)
}

/// Extract the host from a server URL
fn extract_host(server_url: &str) -> Option<&str> {
    let url = server_url
        .strip_prefix("https://")
        .or_else(|| server_url.strip_prefix("http://"))?;
    url.split('/').next()
}

/// Check if the stored credentials match the requested host
fn credentials_match_host(stored_url: &str, requested_host: &str) -> bool {
    if let Some(stored_host) = extract_host(stored_url) {
        // Compare hosts, handling port variations
        let stored_host_base = stored_host.split(':').next().unwrap_or(stored_host);
        let requested_host_base = requested_host.split(':').next().unwrap_or(requested_host);

        stored_host_base == requested_host_base || stored_host == requested_host
    } else {
        false
    }
}

/// Handle `cutman credential get`
pub fn run_credential_get() -> anyhow::Result<()> {
    let attrs = parse_credential_input()?;

    let requested_host = match attrs.get("host") {
        Some(h) => h,
        None => return Ok(()), // No host specified, can't match
    };

    let creds = match load_credentials() {
        Ok(c) => c,
        Err(_) => return Ok(()), // No stored credentials
    };

    if !credentials_match_host(&creds.server_url, requested_host) {
        return Ok(()); // Host doesn't match
    }

    // Output credentials in Git credential format
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Use "cutman" as the username (or could extract from token)
    writeln!(handle, "username=cutman")?;
    writeln!(handle, "password={}", creds.token)?;

    Ok(())
}

/// Handle `cutman credential store`
///
/// This is a no-op because users should use `cutman auth login` to store credentials.
pub fn run_credential_store() -> anyhow::Result<()> {
    // Consume stdin but don't actually store anything
    let _ = parse_credential_input();
    Ok(())
}

/// Handle `cutman credential erase`
///
/// Clears stored credentials if the host matches.
pub fn run_credential_erase() -> anyhow::Result<()> {
    let attrs = parse_credential_input()?;

    let requested_host = match attrs.get("host") {
        Some(h) => h,
        None => return Ok(()), // No host specified
    };

    let creds = match load_credentials() {
        Ok(c) => c,
        Err(_) => return Ok(()), // No stored credentials
    };

    if credentials_match_host(&creds.server_url, requested_host) {
        delete_credentials()?;
    }

    Ok(())
}

/// Print help message for credential command
pub fn print_credential_help() {
    let path = credentials_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.config/cutman/credentials.toml".to_string());

    eprintln!("Git credential helper for cutman.");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  git config --global credential.helper 'cutman credential'");
    eprintln!();
    eprintln!("First, authenticate with:");
    eprintln!("  cutman auth login --server <URL> --token <TOKEN>");
    eprintln!();
    eprintln!("Credentials are stored at: {path}");
    eprintln!();
    eprintln!("Subcommands:");
    eprintln!("  get    - Output credentials for git (reads from stdin)");
    eprintln!("  store  - No-op (use 'cutman auth login' instead)");
    eprintln!("  erase  - Clear credentials if host matches");
}
