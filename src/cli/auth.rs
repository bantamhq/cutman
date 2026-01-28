use inquire::Text;

use super::credentials::{Credentials, delete_credentials, save_credentials};
use super::http_client::{ApiClient, NamespaceWithPrimary};

fn normalize_server_url(url: &str) -> String {
    let url = url.trim().trim_end_matches('/');

    // Strip trailing API paths to avoid duplication when constructing request URLs
    let url = url
        .trim_end_matches("/api/v1")
        .trim_end_matches("/api")
        .trim_end_matches('/');

    if url.starts_with("http://") || url.starts_with("https://") {
        return url.to_string();
    }

    // Default to http:// for localhost/127.0.0.1, https:// for others
    if url.starts_with("localhost") || url.starts_with("127.0.0.1") {
        format!("http://{}", url)
    } else {
        format!("https://{}", url)
    }
}

pub fn run_auth_login(
    server: Option<String>,
    token: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let server = if let Some(s) = server {
        if s.trim().is_empty() {
            anyhow::bail!("Server URL cannot be empty");
        }
        s
    } else if non_interactive {
        anyhow::bail!("--server is required in non-interactive mode");
    } else {
        Text::new("Server URL:")
            .with_validator(|input: &str| {
                if input.trim().is_empty() {
                    Ok(inquire::validator::Validation::Invalid(
                        "Server URL is required".into(),
                    ))
                } else {
                    Ok(inquire::validator::Validation::Valid)
                }
            })
            .prompt()?
    };

    let server_url = normalize_server_url(&server);

    let token = if let Some(t) = token {
        t
    } else if non_interactive {
        anyhow::bail!("--token is required in non-interactive mode");
    } else {
        Text::new("Token:")
            .with_placeholder("cutman_...")
            .prompt()?
    };

    if !token.starts_with("cutman_") {
        anyhow::bail!("Invalid token format. Token should start with 'cutman_'");
    }

    let creds = Credentials {
        server_url: server_url.clone(),
        token,
    };

    let client = ApiClient::new(&creds)?;
    let _namespaces: Vec<NamespaceWithPrimary> = client.get("/namespaces")?;

    save_credentials(&creds)?;

    println!();
    println!("Logged in to {}", server_url);
    println!();

    Ok(())
}

pub fn run_auth_logout() -> anyhow::Result<()> {
    if delete_credentials()? {
        println!();
        println!("Logged out successfully.");
        println!();
    } else {
        println!();
        println!("No credentials found.");
        println!();
    }
    Ok(())
}
