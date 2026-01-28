use std::env;
use std::fs;
use std::process::Command;

use inquire::{Select, Text};
use serde::Serialize;

use super::credentials::{load_credentials, Credentials};
use super::http_client::ApiClient;
use crate::types::{Namespace, Repo};

#[derive(Serialize)]
struct CreateRepoRequest {
    name: String,
    namespace: Option<String>,
    public: bool,
}

pub fn run_new(
    name: Option<String>,
    namespace: Option<String>,
    remote: String,
) -> anyhow::Result<()> {
    let creds = load_credentials()?;
    let client = ApiClient::new(&creds)?;

    let original_dir = env::current_dir()?;

    let (repo_name, work_dir) = if let Some(name) = name {
        let dir = original_dir.join(&name);
        if dir.exists() {
            anyhow::bail!("Directory '{}' already exists", name);
        }
        fs::create_dir(&dir)?;
        (name, dir)
    } else {
        let name = original_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Could not determine folder name"))?
            .to_string();
        (name, original_dir.clone())
    };

    env::set_current_dir(&work_dir)?;

    let result = (|| -> anyhow::Result<()> {
        if !work_dir.join(".git").exists() {
            run_git(&["init"])?;
        }

        let readme_path = work_dir.join("README.md");
        if !readme_path.exists() {
            fs::write(&readme_path, format!("# {}\n", repo_name))?;
        }

        let request = CreateRepoRequest {
            name: repo_name.clone(),
            namespace: namespace.clone(),
            public: false,
        };
        let _repo: Repo = client.post("/repos", &request)?;

        let namespace_name = if let Some(ns) = namespace {
            ns
        } else {
            let namespaces: Vec<Namespace> = client.get("/namespaces")?;
            namespaces
                .into_iter()
                .next()
                .map(|n| n.name)
                .ok_or_else(|| anyhow::anyhow!("No namespace found"))?
        };

        let remote_url = format!(
            "{}/git/{}/{}.git",
            client.base_url(),
            namespace_name,
            repo_name
        );

        let existing_remotes = run_git_output(&["remote"])?;
        let remote = if existing_remotes.lines().any(|r| r == remote) {
            let current_url = run_git_output(&["remote", "get-url", &remote])?;
            let current_url = current_url.trim();

            if current_url == remote_url {
                // Already configured correctly
                remote
            } else {
                // Remote exists with different URL - ask user what to do
                let options = vec![
                    format!("Update '{}' to point to cutman", remote),
                    "Add a new remote with a different name".to_string(),
                    "Cancel".to_string(),
                ];

                println!();
                println!(
                    "Remote '{}' already exists pointing to:",
                    remote
                );
                println!("  {}", current_url);
                println!();

                let choice = Select::new("What would you like to do?", options)
                    .with_vim_mode(true)
                    .prompt()?;

                if choice.starts_with("Update") {
                    run_git(&["remote", "set-url", &remote, &remote_url])?;
                    remote
                } else if choice.starts_with("Add") {
                    let new_remote = Text::new("Remote name:")
                        .with_placeholder("cutman")
                        .with_default("cutman")
                        .prompt()?;

                    if existing_remotes.lines().any(|r| r == new_remote) {
                        anyhow::bail!("Remote '{}' already exists", new_remote);
                    }

                    run_git(&["remote", "add", &new_remote, &remote_url])?;
                    new_remote
                } else {
                    anyhow::bail!("Cancelled");
                }
            }
        } else {
            run_git(&["remote", "add", &remote, &remote_url])?;
            remote
        };

        run_git(&["add", "."])?;

        let status = run_git_output(&["status", "--porcelain"])?;
        if !status.is_empty() {
            run_git(&["commit", "-m", "Initial commit"])?;
        }

        let branch = run_git_output(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        let branch = branch.trim();

        run_git_with_auth(&["push", "-u", &remote, branch], &creds)?;

        println!();
        println!("Created repository '{}'", repo_name);
        println!("Remote: {}", remote_url);
        println!();

        Ok(())
    })();

    env::set_current_dir(&original_dir)?;

    result
}

fn run_git(args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new("git").args(args).status()?;
    if !status.success() {
        anyhow::bail!("git {} failed", args.join(" "));
    }
    Ok(())
}

fn run_git_with_auth(args: &[&str], creds: &Credentials) -> anyhow::Result<()> {
    let auth_header = format!("http.extraHeader=Authorization: Bearer {}", creds.token);
    let status = Command::new("git")
        .arg("-c")
        .arg(&auth_header)
        .args(args)
        .status()?;
    if !status.success() {
        anyhow::bail!("git {} failed", args.join(" "));
    }
    Ok(())
}

fn run_git_output(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git").args(args).output()?;
    if !output.status.success() {
        anyhow::bail!("git {} failed", args.join(" "));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
