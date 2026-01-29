//! CLI integration tests for cutman admin commands.
//!
//! Each test uses an isolated temp directory for the database, ensuring tests
//! can run in parallel safely.

#![allow(deprecated)] // Command::cargo_bin deprecation only affects custom build dirs

use std::path::Path;

use assert_cmd::Command;
use assert_fs::TempDir;
use chrono::Utc;
use cutman::store::{SqliteStore, Store};
use cutman::types::Repo;
use predicates::prelude::*;
use serde_json::Value;
use uuid::Uuid;

struct TestContext {
    temp_dir: TempDir,
}

impl TestContext {
    fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("failed to create temp dir"),
        }
    }

    fn data_dir(&self) -> &Path {
        self.temp_dir.path()
    }

    fn data_dir_str(&self) -> String {
        self.data_dir().to_string_lossy().to_string()
    }

    fn init(&self) -> assert_cmd::assert::Assert {
        Command::cargo_bin("cutman")
            .expect("failed to find binary")
            .args([
                "admin",
                "init",
                "--data-dir",
                &self.data_dir_str(),
                "--non-interactive",
            ])
            .assert()
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("cutman").expect("failed to find binary");
        cmd.env("NO_COLOR", "1");
        cmd
    }

    fn info_json(&self) -> Value {
        let output = self
            .cmd()
            .args([
                "admin",
                "info",
                "--data-dir",
                &self.data_dir_str(),
                "--json",
            ])
            .output()
            .expect("failed to run command");

        serde_json::from_slice(&output.stdout).expect("failed to parse JSON")
    }

    fn remove_user(&self, user_id: &str) -> assert_cmd::assert::Assert {
        self.cmd()
            .args([
                "admin",
                "user",
                "remove",
                "--data-dir",
                &self.data_dir_str(),
                "--user-id",
                user_id,
                "--non-interactive",
                "--yes",
            ])
            .assert()
    }

    fn remove_namespace(&self, namespace_id: &str) -> assert_cmd::assert::Assert {
        self.cmd()
            .args([
                "admin",
                "namespace",
                "remove",
                "--data-dir",
                &self.data_dir_str(),
                "--namespace-id",
                namespace_id,
                "--non-interactive",
                "--yes",
            ])
            .assert()
    }
}

fn find_id_by_field<'a>(items: &'a [Value], field: &str, value: &str) -> &'a str {
    items
        .iter()
        .find(|item| item[field] == value)
        .expect("item not found")["id"]
        .as_str()
        .expect("id not a string")
}

fn add_user(ctx: &TestContext, username: &str) -> String {
    ctx.cmd()
        .args([
            "admin",
            "user",
            "add",
            "--data-dir",
            &ctx.data_dir_str(),
            "--username",
            username,
            "--non-interactive",
        ])
        .assert()
        .success();

    get_user_id(ctx, username)
}

fn get_user_id(ctx: &TestContext, username: &str) -> String {
    let info = ctx.info_json();
    let users = info["users"].as_array().expect("users not an array");
    find_id_by_field(users, "username", username).to_string()
}

fn add_namespace(ctx: &TestContext, name: &str) -> String {
    ctx.cmd()
        .args([
            "admin",
            "namespace",
            "add",
            "--data-dir",
            &ctx.data_dir_str(),
            "--name",
            name,
            "--non-interactive",
        ])
        .assert()
        .success();

    get_namespace_id(ctx, name)
}

fn get_namespace_id(ctx: &TestContext, name: &str) -> String {
    let info = ctx.info_json();
    let namespaces = info["namespaces"]
        .as_array()
        .expect("namespaces not an array");
    find_id_by_field(namespaces, "name", name).to_string()
}

fn create_token(ctx: &TestContext, user_id: &str) -> String {
    ctx.cmd()
        .args([
            "admin",
            "token",
            "create",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            user_id,
            "--non-interactive",
        ])
        .assert()
        .success();

    let info = ctx.info_json();
    let tokens = info["tokens"].as_array().expect("tokens not an array");
    find_last_token_for_user(tokens, user_id)["id"]
        .as_str()
        .expect("id not a string")
        .to_string()
}

fn grant_permission(ctx: &TestContext, user_id: &str, namespace_id: &str, perms: &str) {
    ctx.cmd()
        .args([
            "admin",
            "permission",
            "grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            user_id,
            "--namespace-id",
            namespace_id,
            "--permissions",
            perms,
            "--non-interactive",
        ])
        .assert()
        .success();
}

fn list_tokens_json(ctx: &TestContext) -> Vec<Value> {
    let info = ctx.info_json();
    info["tokens"]
        .as_array()
        .expect("tokens not an array")
        .clone()
}

fn list_grants_json(ctx: &TestContext) -> Vec<Value> {
    let info = ctx.info_json();
    info["grants"]
        .as_array()
        .expect("grants not an array")
        .clone()
}

fn list_users_json(ctx: &TestContext) -> Vec<Value> {
    let info = ctx.info_json();
    info["users"]
        .as_array()
        .expect("users not an array")
        .clone()
}

fn list_namespaces_json(ctx: &TestContext) -> Vec<Value> {
    let info = ctx.info_json();
    info["namespaces"]
        .as_array()
        .expect("namespaces not an array")
        .clone()
}

fn find_last_token_for_user<'a>(tokens: &'a [Value], user_id: &str) -> &'a Value {
    tokens
        .iter()
        .rfind(|t| t["user_id"].as_str() == Some(user_id))
        .expect("token not found")
}

fn open_store(ctx: &TestContext) -> SqliteStore {
    let db_path = ctx.data_dir().join("cutman.db");
    SqliteStore::new(&db_path).expect("open store")
}

fn create_repo(ctx: &TestContext, namespace_id: &str, name: &str) -> String {
    let now = Utc::now();
    let repo = Repo {
        id: Uuid::new_v4().to_string(),
        namespace_id: namespace_id.to_string(),
        name: name.to_string(),
        description: None,
        public: false,
        size_bytes: 0,
        folder_id: None,
        last_push_at: None,
        created_at: now,
        updated_at: now,
    };
    let store = open_store(ctx);
    store.create_repo(&repo).expect("create repo");
    repo.id
}

// ============================================================================
// Init Command Tests
// ============================================================================

#[test]
fn init_creates_database_file_and_admin_token_file() {
    let ctx = TestContext::new();

    ctx.init().success();

    assert!(ctx.data_dir().join("cutman.db").exists());
    assert!(ctx.data_dir().join(".admin_token").exists());

    let token_content = std::fs::read_to_string(ctx.data_dir().join(".admin_token"))
        .expect("failed to read token file");
    assert!(token_content.starts_with("cutman_"));
}

#[test]
fn init_rejects_second_initialization_with_existing_database() {
    let ctx = TestContext::new();

    ctx.init().success();
    ctx.init()
        .failure()
        .stderr(predicate::str::contains("already initialized"));
}

#[test]
fn init_preserves_existing_users_when_reinitialization_rejected() {
    let ctx = TestContext::new();

    ctx.init().success();
    add_user(&ctx, "testuser");

    ctx.init().failure();

    let users = list_users_json(&ctx);
    assert!(users.iter().any(|u| u["username"] == "testuser"));
}

// ============================================================================
// User Cascading Deletion Tests
// ============================================================================

#[test]
fn user_remove_deletes_all_tokens_belonging_to_user() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "alice");
    create_token(&ctx, &user_id);
    create_token(&ctx, &user_id);

    let count_user_tokens = |tokens: &[Value]| {
        tokens
            .iter()
            .filter(|t| t["user_id"].as_str() == Some(&user_id))
            .count()
    };

    assert_eq!(count_user_tokens(&list_tokens_json(&ctx)), 2);

    ctx.remove_user(&user_id).success();

    assert_eq!(count_user_tokens(&list_tokens_json(&ctx)), 0);
}

#[test]
fn user_remove_deletes_all_namespace_grants_for_user() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "bob");
    let ns_id = add_namespace(&ctx, "shared");
    grant_permission(&ctx, &user_id, &ns_id, "repo:read");

    let has_user_grant = |grants: &[Value]| grants.iter().any(|g| g["user_id"] == user_id);

    assert!(has_user_grant(&list_grants_json(&ctx)));

    ctx.remove_user(&user_id).success();

    assert!(!has_user_grant(&list_grants_json(&ctx)));
}

#[test]
fn user_remove_deletes_users_primary_namespace() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "carol");

    let has_namespace = |namespaces: &[Value]| namespaces.iter().any(|n| n["name"] == "carol");

    assert!(has_namespace(&list_namespaces_json(&ctx)));

    ctx.remove_user(&user_id).success();

    assert!(!has_namespace(&list_namespaces_json(&ctx)));
}

#[test]
fn user_remove_leaves_other_users_tokens_and_grants_intact() {
    let ctx = TestContext::new();
    ctx.init().success();

    let alice_id = add_user(&ctx, "alice");
    let bob_id = add_user(&ctx, "bob");
    let ns_id = add_namespace(&ctx, "shared");

    create_token(&ctx, &alice_id);
    create_token(&ctx, &bob_id);
    grant_permission(&ctx, &alice_id, &ns_id, "repo:read");
    grant_permission(&ctx, &bob_id, &ns_id, "repo:write");

    ctx.remove_user(&alice_id).success();

    let tokens = list_tokens_json(&ctx);
    assert!(
        tokens
            .iter()
            .any(|t| t["user_id"].as_str() == Some(&bob_id))
    );

    let grants = list_grants_json(&ctx);
    assert!(grants.iter().any(|g| g["user_id"] == bob_id));
}

// ============================================================================
// Namespace Primary Guard Tests
// ============================================================================

#[test]
fn namespace_remove_rejects_deletion_of_users_primary_namespace() {
    let ctx = TestContext::new();
    ctx.init().success();

    add_user(&ctx, "dave");
    let ns_id = get_namespace_id(&ctx, "dave");

    ctx.remove_namespace(&ns_id)
        .failure()
        .stderr(predicate::str::contains("primary namespace"));
}

#[test]
fn namespace_remove_allows_deletion_of_shared_namespace() {
    let ctx = TestContext::new();
    ctx.init().success();

    let ns_id = add_namespace(&ctx, "shared");

    ctx.remove_namespace(&ns_id).success();

    let namespaces = list_namespaces_json(&ctx);
    assert!(!namespaces.iter().any(|n| n["name"] == "shared"));
}

// ============================================================================
// Permission Parsing Tests
// ============================================================================

#[test]
fn permission_grant_accepts_single_permission_string() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "eve");
    let ns_id = add_namespace(&ctx, "shared");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--namespace-id",
            &ns_id,
            "--permissions",
            "repo:read",
            "--non-interactive",
        ])
        .assert()
        .success();

    let grants = list_grants_json(&ctx);
    let grant = grants
        .iter()
        .find(|g| g["user_id"] == user_id)
        .expect("grant not found");
    let perms = grant["permissions"]
        .as_array()
        .expect("permissions not an array");
    assert!(perms.iter().any(|p| p == "repo:read"));
}

#[test]
fn permission_grant_parses_comma_separated_permissions() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "frank");
    let ns_id = add_namespace(&ctx, "shared");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--namespace-id",
            &ns_id,
            "--permissions",
            "repo:read,repo:write,namespace:read",
            "--non-interactive",
        ])
        .assert()
        .success();

    let grants = list_grants_json(&ctx);
    let grant = grants
        .iter()
        .find(|g| g["user_id"] == user_id)
        .expect("grant not found");
    let perms = grant["permissions"]
        .as_array()
        .expect("permissions not an array");
    assert!(perms.iter().any(|p| p == "repo:read"));
    assert!(perms.iter().any(|p| p == "repo:write"));
    assert!(perms.iter().any(|p| p == "namespace:read"));
}

#[test]
fn permission_grant_trims_whitespace_around_permissions() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "grace");
    let ns_id = add_namespace(&ctx, "shared");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--namespace-id",
            &ns_id,
            "--permissions",
            " repo:read , repo:write ",
            "--non-interactive",
        ])
        .assert()
        .success();

    let grants = list_grants_json(&ctx);
    let grant = grants
        .iter()
        .find(|g| g["user_id"] == user_id)
        .expect("grant not found");
    let perms = grant["permissions"]
        .as_array()
        .expect("permissions not an array");
    assert!(perms.iter().any(|p| p == "repo:read"));
    assert!(perms.iter().any(|p| p == "repo:write"));
}

#[test]
fn permission_grant_rejects_invalid_permission_name() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "henry");
    let ns_id = add_namespace(&ctx, "shared");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--namespace-id",
            &ns_id,
            "--permissions",
            "invalid:permission",
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid permission"));
}

// ============================================================================
// Token Expiration Tests
// ============================================================================

#[test]
fn token_create_with_expires_days_sets_expiration_date() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "irene");

    ctx.cmd()
        .args([
            "admin",
            "token",
            "create",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--expires-days",
            "30",
            "--non-interactive",
        ])
        .assert()
        .success();

    let tokens = list_tokens_json(&ctx);
    let token = find_last_token_for_user(&tokens, &user_id);
    assert!(token["expires_at"].is_string());
}

#[test]
fn token_create_with_zero_expires_days_creates_non_expiring_token() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "jack");

    ctx.cmd()
        .args([
            "admin",
            "token",
            "create",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--expires-days",
            "0",
            "--non-interactive",
        ])
        .assert()
        .success();

    let tokens = list_tokens_json(&ctx);
    let token = find_last_token_for_user(&tokens, &user_id);
    assert!(token["expires_at"].is_null());
}

#[test]
fn token_create_without_expires_days_defaults_to_non_expiring() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "kate");

    ctx.cmd()
        .args([
            "admin",
            "token",
            "create",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--non-interactive",
        ])
        .assert()
        .success();

    let tokens = list_tokens_json(&ctx);
    let token = find_last_token_for_user(&tokens, &user_id);
    assert!(token["expires_at"].is_null());
}

// ============================================================================
// Non-Interactive Validation Tests
// ============================================================================

#[test]
fn user_add_in_non_interactive_mode_fails_without_username_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    ctx.cmd()
        .args([
            "admin",
            "user",
            "add",
            "--data-dir",
            &ctx.data_dir_str(),
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--username is required"));
}

#[test]
fn user_remove_in_non_interactive_mode_fails_without_user_id_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    add_user(&ctx, "leo");

    ctx.cmd()
        .args([
            "admin",
            "user",
            "remove",
            "--data-dir",
            &ctx.data_dir_str(),
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--user-id is required"));
}

#[test]
fn user_remove_in_non_interactive_mode_fails_without_yes_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "mike");

    ctx.cmd()
        .args([
            "admin",
            "user",
            "remove",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}

#[test]
fn permission_grant_in_non_interactive_mode_fails_without_permissions_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "nancy");
    let ns_id = add_namespace(&ctx, "shared");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--namespace-id",
            &ns_id,
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--permissions is required"));
}

// ============================================================================
// JSON Output Tests
// ============================================================================

#[test]
fn info_with_json_flag_outputs_valid_json() {
    let ctx = TestContext::new();
    ctx.init().success();

    let output = ctx
        .cmd()
        .args(["admin", "info", "--data-dir", &ctx.data_dir_str(), "--json"])
        .output()
        .expect("failed to run command");

    let _: Value = serde_json::from_slice(&output.stdout).expect("output is not valid JSON");
}

#[test]
fn info_json_output_contains_users_namespaces_tokens_and_repos_fields() {
    let ctx = TestContext::new();
    ctx.init().success();

    let output = ctx
        .cmd()
        .args(["admin", "info", "--data-dir", &ctx.data_dir_str(), "--json"])
        .output()
        .expect("failed to run command");

    let info: Value = serde_json::from_slice(&output.stdout).expect("output is not valid JSON");

    assert!(info.get("users").is_some(), "missing 'users' field");
    assert!(
        info.get("namespaces").is_some(),
        "missing 'namespaces' field"
    );
    assert!(info.get("tokens").is_some(), "missing 'tokens' field");
    assert!(info.get("repos").is_some(), "missing 'repos' field");
}

// ==========================================================================
// Additional Non-Interactive Validation Tests
// ==========================================================================

#[test]
fn token_create_in_non_interactive_mode_fails_without_user_id_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    ctx.cmd()
        .args([
            "admin",
            "token",
            "create",
            "--data-dir",
            &ctx.data_dir_str(),
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--user-id is required"));
}

#[test]
fn token_revoke_in_non_interactive_mode_fails_without_token_id_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    ctx.cmd()
        .args([
            "admin",
            "token",
            "revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--non-interactive",
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--token-id is required"));
}

#[test]
fn token_revoke_in_non_interactive_mode_fails_without_yes_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "oliver");
    let token_id = create_token(&ctx, &user_id);

    ctx.cmd()
        .args([
            "admin",
            "token",
            "revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--token-id",
            &token_id,
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes is required"));
}

#[test]
fn token_revoke_removes_token() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "paul");
    let token_id = create_token(&ctx, &user_id);

    ctx.cmd()
        .args([
            "admin",
            "token",
            "revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--token-id",
            &token_id,
            "--non-interactive",
            "--yes",
        ])
        .assert()
        .success();

    let tokens = list_tokens_json(&ctx);
    assert!(tokens.iter().all(|t| t["id"] != token_id));
}

#[test]
fn namespace_add_in_non_interactive_mode_fails_without_name_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    ctx.cmd()
        .args([
            "admin",
            "namespace",
            "add",
            "--data-dir",
            &ctx.data_dir_str(),
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--name is required"));
}

#[test]
fn namespace_remove_in_non_interactive_mode_fails_without_namespace_id_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    ctx.cmd()
        .args([
            "admin",
            "namespace",
            "remove",
            "--data-dir",
            &ctx.data_dir_str(),
            "--non-interactive",
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--namespace-id is required"));
}

#[test]
fn namespace_remove_in_non_interactive_mode_fails_without_yes_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let ns_id = add_namespace(&ctx, "shared-no-yes");

    ctx.cmd()
        .args([
            "admin",
            "namespace",
            "remove",
            "--data-dir",
            &ctx.data_dir_str(),
            "--namespace-id",
            &ns_id,
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes is required"));
}

#[test]
fn permission_revoke_in_non_interactive_mode_fails_without_namespace_id_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "quinn");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--non-interactive",
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--namespace-id is required"));
}

#[test]
fn permission_revoke_in_non_interactive_mode_fails_without_yes_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "riley");
    let ns_id = add_namespace(&ctx, "shared-revoke");
    grant_permission(&ctx, &user_id, &ns_id, "repo:read");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--namespace-id",
            &ns_id,
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes is required"));
}

#[test]
fn permission_revoke_removes_grant() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "sam");
    let ns_id = add_namespace(&ctx, "shared-grant");
    grant_permission(&ctx, &user_id, &ns_id, "repo:read");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--namespace-id",
            &ns_id,
            "--non-interactive",
            "--yes",
        ])
        .assert()
        .success();

    let grants = list_grants_json(&ctx);
    assert!(grants.iter().all(|g| g["user_id"] != user_id));
}

#[test]
fn permission_repo_grant_in_non_interactive_mode_fails_without_repo_id_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "taylor");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "repo-grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--permissions",
            "repo:read",
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--repo-id is required"));
}

#[test]
fn permission_repo_grant_in_non_interactive_mode_fails_without_permissions_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "uma");
    let ns_id = get_namespace_id(&ctx, "uma");
    let repo_id = create_repo(&ctx, &ns_id, "repo-for-perms");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "repo-grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--repo-id",
            &repo_id,
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--permissions is required"));
}

#[test]
fn permission_repo_revoke_in_non_interactive_mode_fails_without_repo_id_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "vera");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "repo-revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--non-interactive",
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--repo-id is required"));
}

#[test]
fn permission_repo_revoke_in_non_interactive_mode_fails_without_yes_flag() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "wren");
    let ns_id = get_namespace_id(&ctx, "wren");
    let repo_id = create_repo(&ctx, &ns_id, "repo-for-revoke");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "repo-grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--repo-id",
            &repo_id,
            "--permissions",
            "repo:read",
            "--non-interactive",
        ])
        .assert()
        .success();

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "repo-revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--repo-id",
            &repo_id,
            "--non-interactive",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes is required"));
}

#[test]
fn permission_repo_grant_and_revoke_work() {
    let ctx = TestContext::new();
    ctx.init().success();

    let user_id = add_user(&ctx, "xena");
    let ns_id = get_namespace_id(&ctx, "xena");
    let repo_id = create_repo(&ctx, &ns_id, "repo-access");

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "repo-grant",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--repo-id",
            &repo_id,
            "--permissions",
            "repo:read",
            "--non-interactive",
        ])
        .assert()
        .success();

    let store = open_store(&ctx);
    assert!(
        store
            .get_repo_grant(&user_id, &repo_id)
            .expect("get repo grant")
            .is_some()
    );

    ctx.cmd()
        .args([
            "admin",
            "permission",
            "repo-revoke",
            "--data-dir",
            &ctx.data_dir_str(),
            "--user-id",
            &user_id,
            "--repo-id",
            &repo_id,
            "--non-interactive",
            "--yes",
        ])
        .assert()
        .success();

    let store = open_store(&ctx);
    assert!(
        store
            .get_repo_grant(&user_id, &repo_id)
            .expect("get repo grant")
            .is_none()
    );
}

// ============================================================================
// Serve Command Tests
// ============================================================================

#[test]
fn serve_requires_initialization() {
    let temp_dir = TempDir::new().expect("failed to create temp dir");

    Command::cargo_bin("cutman")
        .expect("failed to find binary")
        .args(["serve", "--data-dir"])
        .arg(temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Server not initialized"));
}
