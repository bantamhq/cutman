#![allow(deprecated)]

mod common;

use assert_cmd::Command;
use assert_fs::TempDir;
use predicates::prelude::*;
use reqwest::{Client, StatusCode};
use serde_json::Value;
use std::process::Command as ProcessCommand;

use common::TestServer;

fn cli_cmd(config_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("cutman").expect("failed to find binary");
    cmd.env("NO_COLOR", "1");
    cmd.env("HOME", config_dir.path());
    cmd.env("XDG_CONFIG_HOME", config_dir.path());
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

fn git_available() -> bool {
    ProcessCommand::new("git").arg("--version").output().is_ok()
}

struct PrincipalSetup {
    principal_ns_id: String,
    principal_ns_name: String,
    principal_token: String,
}

async fn create_principal_and_token(
    client: &Client,
    base_url: &str,
    admin_token: &str,
    namespace_name: &str,
) -> PrincipalSetup {
    let resp: Value = client
        .post(format!("{}/api/v1/admin/principals", base_url))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"namespace_name": namespace_name}))
        .send()
        .await
        .expect("create principal")
        .json()
        .await
        .expect("parse principal response");

    let principal_id = resp["data"]["id"]
        .as_str()
        .expect("principal id")
        .to_string();
    let principal_ns_id = resp["data"]["primary_namespace_id"]
        .as_str()
        .expect("principal namespace id")
        .to_string();

    let token_resp: Value = client
        .post(format!(
            "{}/api/v1/admin/principals/{}/tokens",
            base_url, principal_id
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"description": "CLI test token"}))
        .send()
        .await
        .expect("create principal token")
        .json()
        .await
        .expect("parse token response");

    let principal_token = token_resp["data"]["token"]
        .as_str()
        .expect("principal token")
        .to_string();

    PrincipalSetup {
        principal_ns_id,
        principal_ns_name: namespace_name.to_string(),
        principal_token,
    }
}

async fn create_repo(
    client: &Client,
    base_url: &str,
    principal_token: &str,
    repo_name: &str,
    namespace: &str,
) -> String {
    let resp: Value = client
        .post(format!("{}/api/v1/repos", base_url))
        .bearer_auth(principal_token)
        .json(&serde_json::json!({
            "name": repo_name,
            "namespace": namespace,
        }))
        .send()
        .await
        .expect("create repo")
        .json()
        .await
        .expect("parse repo response");

    resp["data"]["id"].as_str().expect("repo id").to_string()
}

async fn create_tag(
    client: &Client,
    base_url: &str,
    principal_token: &str,
    tag_name: &str,
    namespace: &str,
) -> String {
    let resp: Value = client
        .post(format!("{}/api/v1/tags", base_url))
        .bearer_auth(principal_token)
        .json(&serde_json::json!({
            "name": tag_name,
            "namespace": namespace,
        }))
        .send()
        .await
        .expect("create tag")
        .json()
        .await
        .expect("parse tag response");

    resp["data"]["id"].as_str().expect("tag id").to_string()
}

async fn list_repo_tag_ids(
    client: &Client,
    base_url: &str,
    principal_token: &str,
    repo_id: &str,
) -> Vec<String> {
    let resp: Value = client
        .get(format!("{}/api/v1/repos/{}/tags", base_url, repo_id))
        .bearer_auth(principal_token)
        .send()
        .await
        .expect("list repo tags")
        .json()
        .await
        .expect("parse repo tags response");

    resp["data"]
        .as_array()
        .expect("tags array")
        .iter()
        .filter_map(|tag| tag["id"].as_str().map(|id| id.to_string()))
        .collect()
}

#[test]
fn auth_login_requires_server_in_non_interactive_mode() {
    let config_dir = TempDir::new().expect("failed to create temp dir");

    cli_cmd(&config_dir)
        .args([
            "auth",
            "login",
            "--token",
            "cutman_test",
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--server is required"));
}

#[test]
fn auth_login_requires_token_in_non_interactive_mode() {
    let config_dir = TempDir::new().expect("failed to create temp dir");

    cli_cmd(&config_dir)
        .args([
            "auth",
            "login",
            "--server",
            "http://localhost",
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--token is required"));
}

#[test]
fn auth_login_rejects_invalid_token_format() {
    let config_dir = TempDir::new().expect("failed to create temp dir");

    cli_cmd(&config_dir)
        .args([
            "auth",
            "login",
            "--server",
            "http://localhost",
            "--token",
            "invalid",
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid token format"));
}

#[tokio::test]
async fn auth_login_and_credential_helper_roundtrip() {
    let server = TestServer::start().await;
    let client = Client::new();

    let principal =
        create_principal_and_token(&client, &server.base_url, &server.admin_token, "cli-user")
            .await;

    let config_dir = TempDir::new().expect("failed to create temp dir");

    cli_cmd(&config_dir)
        .args([
            "auth",
            "login",
            "--server",
            &server.base_url,
            "--token",
            &principal.principal_token,
            "--non-interactive",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Logged in to"));

    let host = server
        .base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string();

    let output = cli_cmd(&config_dir)
        .args(["credential", "get"])
        .write_stdin(format!("host={}\n\n", host))
        .output()
        .expect("credential get");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("username=cutman"));
    assert!(stdout.contains(&format!("password={}", principal.principal_token)));

    let output = cli_cmd(&config_dir)
        .args(["credential", "get"])
        .write_stdin("host=example.com\n\n".to_string())
        .output()
        .expect("credential get mismatched");
    assert!(output.status.success());
    assert!(output.stdout.is_empty());

    cli_cmd(&config_dir)
        .args(["credential", "erase"])
        .write_stdin(format!("host={}\n\n", host))
        .assert()
        .success();

    let output = cli_cmd(&config_dir)
        .args(["credential", "get"])
        .write_stdin(format!("host={}\n\n", host))
        .output()
        .expect("credential get after erase");
    assert!(output.status.success());
    assert!(output.stdout.is_empty());

    cli_cmd(&config_dir)
        .args([
            "auth",
            "login",
            "--server",
            &server.base_url,
            "--token",
            &principal.principal_token,
            "--non-interactive",
        ])
        .assert()
        .success();

    cli_cmd(&config_dir)
        .args(["auth", "logout"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Logged out successfully"));
}

#[tokio::test]
async fn repo_tag_and_delete_flow() {
    let server = TestServer::start().await;
    let client = Client::new();

    let principal = create_principal_and_token(
        &client,
        &server.base_url,
        &server.admin_token,
        "cli-repo-user",
    )
    .await;

    let config_dir = TempDir::new().expect("failed to create temp dir");

    cli_cmd(&config_dir)
        .args([
            "auth",
            "login",
            "--server",
            &server.base_url,
            "--token",
            &principal.principal_token,
            "--non-interactive",
        ])
        .assert()
        .success();

    let repo_name = "cli-repo";
    let repo_id = create_repo(
        &client,
        &server.base_url,
        &principal.principal_token,
        repo_name,
        &principal.principal_ns_name,
    )
    .await;
    let repo_ref = format!("{}/{}", principal.principal_ns_name, repo_name);

    cli_cmd(&config_dir)
        .args(["repo", "tag", "--non-interactive", &repo_ref])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--tags is required"));

    let tag_a = create_tag(
        &client,
        &server.base_url,
        &principal.principal_token,
        "cli-tag-a",
        &principal.principal_ns_name,
    )
    .await;
    let tag_b = create_tag(
        &client,
        &server.base_url,
        &principal.principal_token,
        "cli-tag-b",
        &principal.principal_ns_name,
    )
    .await;

    let tags_arg = format!("{},{}", tag_a, tag_b);
    cli_cmd(&config_dir)
        .args([
            "repo",
            "tag",
            "--tags",
            &tags_arg,
            "--non-interactive",
            &repo_ref,
        ])
        .assert()
        .success();

    let mut tag_ids =
        list_repo_tag_ids(&client, &server.base_url, &principal.principal_token, &repo_id).await;
    tag_ids.sort();
    let mut expected = vec![tag_a.clone(), tag_b.clone()];
    expected.sort();
    assert_eq!(tag_ids, expected);

    cli_cmd(&config_dir)
        .args(["repo", "delete", "--non-interactive", "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Repository argument is required"));

    cli_cmd(&config_dir)
        .args(["repo", "delete", "--non-interactive", "--yes", &repo_ref])
        .assert()
        .success();

    let resp = client
        .get(format!("{}/api/v1/repos/{}", server.base_url, repo_id))
        .bearer_auth(&principal.principal_token)
        .send()
        .await
        .expect("check repo deleted");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tag_create_and_delete_flow() {
    let server = TestServer::start().await;
    let client = Client::new();

    let principal = create_principal_and_token(
        &client,
        &server.base_url,
        &server.admin_token,
        "cli-tag-user",
    )
    .await;

    let config_dir = TempDir::new().expect("failed to create temp dir");

    cli_cmd(&config_dir)
        .args([
            "auth",
            "login",
            "--server",
            &server.base_url,
            "--token",
            &principal.principal_token,
            "--non-interactive",
        ])
        .assert()
        .success();

    cli_cmd(&config_dir)
        .args(["tag", "create", "--non-interactive"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--name is required"));

    cli_cmd(&config_dir)
        .args([
            "tag",
            "create",
            "--name",
            "cli-tag",
            "--namespace",
            &principal.principal_ns_name,
            "--non-interactive",
        ])
        .assert()
        .success();

    let tags_resp: Value = client
        .get(format!(
            "{}/api/v1/tags?namespace={}",
            server.base_url, principal.principal_ns_name
        ))
        .bearer_auth(&principal.principal_token)
        .send()
        .await
        .expect("list tags")
        .json()
        .await
        .expect("parse tags response");

    let tag_id = tags_resp["data"]
        .as_array()
        .expect("tags array")
        .iter()
        .find(|tag| tag["name"] == "cli-tag")
        .and_then(|tag| tag["id"].as_str())
        .expect("tag id")
        .to_string();

    cli_cmd(&config_dir)
        .args([
            "tag",
            "delete",
            "--tag-id",
            &tag_id,
            "--non-interactive",
            "--yes",
        ])
        .assert()
        .success();

    let resp = client
        .get(format!("{}/api/v1/tags/{}", server.base_url, tag_id))
        .bearer_auth(&principal.principal_token)
        .send()
        .await
        .expect("check tag deleted");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn repo_clone_clones_bare_repo() {
    if !git_available() {
        eprintln!("Skipping repo clone test: git not available");
        return;
    }

    let server = TestServer::start().await;
    let client = Client::new();

    let principal = create_principal_and_token(
        &client,
        &server.base_url,
        &server.admin_token,
        "cli-clone-user",
    )
    .await;

    let config_dir = TempDir::new().expect("failed to create temp dir");

    cli_cmd(&config_dir)
        .args([
            "auth",
            "login",
            "--server",
            &server.base_url,
            "--token",
            &principal.principal_token,
            "--non-interactive",
        ])
        .assert()
        .success();

    let repo_name = "cli-clone-repo";
    let _repo_id = create_repo(
        &client,
        &server.base_url,
        &principal.principal_token,
        repo_name,
        &principal.principal_ns_name,
    )
    .await;

    let bare_repo_path = server
        .data_dir()
        .join("repos")
        .join(&principal.principal_ns_id)
        .join(format!("{repo_name}.git"));
    std::fs::create_dir_all(bare_repo_path.parent().expect("bare repo parent"))
        .expect("create bare repo parent");

    let init_status = ProcessCommand::new("git")
        .args(["init", "--bare"])
        .arg(&bare_repo_path)
        .status()
        .expect("init bare repo");
    assert!(init_status.success());
    std::fs::write(bare_repo_path.join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");

    let work_dir = TempDir::new().expect("failed to create temp dir");
    let repo_ref = format!("{}/{}", principal.principal_ns_name, repo_name);

    cli_cmd(&config_dir)
        .current_dir(work_dir.path())
        .args(["repo", "clone", "--non-interactive", &repo_ref])
        .assert()
        .success();

    assert!(work_dir.path().join(repo_name).join(".git").exists());
}

#[test]
fn new_requires_login() {
    let config_dir = TempDir::new().expect("failed to create temp dir");
    let work_dir = TempDir::new().expect("failed to create temp dir");

    cli_cmd(&config_dir)
        .current_dir(work_dir.path())
        .args(["new", "test-repo"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not logged in"));
}
