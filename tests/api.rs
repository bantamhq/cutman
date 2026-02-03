mod common;

use std::process::Command;

use base64::Engine;
use serde_json::Value;

struct TestData {
    namespace_id: String,
    principal_id: String,
    principal_ns_id: String,
    principal_token: String,
    token_id: String,
    repo_id: String,
    commit_sha: String,
    git_auth_header: String,
    test_suffix: String,
}

impl TestData {
    fn write_vars_file(&self, path: &std::path::Path, base_url: &str, admin_token: &str) {
        let lines = [
            format!("base_url={}", base_url),
            format!("admin_token={}", admin_token),
            format!("principal_token={}", self.principal_token),
            format!("namespace_id={}", self.namespace_id),
            "namespace_name=test-namespace".to_string(),
            format!("principal_id={}", self.principal_id),
            format!("principal_ns_id={}", self.principal_ns_id),
            "principal_ns_name=test-principal".to_string(),
            format!("repo_id={}", self.repo_id),
            "repo_name=test-repo".to_string(),
            format!("token_id={}", self.token_id),
            format!("commit_sha={}", self.commit_sha),
            format!("git_auth_header={}", self.git_auth_header),
            format!("test_suffix={}", self.test_suffix),
        ];
        std::fs::write(path, lines.join("\n") + "\n").expect("write vars file");
    }
}

async fn create_test_data(
    base_url: &str,
    admin_token: &str,
    data_dir: &std::path::Path,
) -> TestData {
    let client = reqwest::Client::new();

    let resp: Value = client
        .post(format!("{}/api/v1/admin/namespaces", base_url))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"name": "test-namespace", "repo_limit": 100}))
        .send()
        .await
        .expect("create namespace")
        .json()
        .await
        .expect("parse namespace response");
    let namespace_id = resp["data"]["id"]
        .as_str()
        .expect("namespace id")
        .to_string();

    let resp: Value = client
        .post(format!("{}/api/v1/admin/principals", base_url))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"namespace_name": "test-principal"}))
        .send()
        .await
        .expect("create principal")
        .json()
        .await
        .expect("parse principal response");
    let principal_id = resp["data"]["id"].as_str().expect("principal id").to_string();
    let principal_ns_id = resp["data"]["primary_namespace_id"]
        .as_str()
        .expect("principal ns id")
        .to_string();

    client
        .post(format!(
            "{}/api/v1/admin/principals/{}/namespace-grants",
            base_url, principal_id
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({
            "namespace_id": namespace_id,
            "allow": ["namespace:read", "namespace:write", "repo:read", "repo:write", "repo:admin"]
        }))
        .send()
        .await
        .expect("grant namespace access");

    let resp: Value = client
        .post(format!(
            "{}/api/v1/admin/principals/{}/tokens",
            base_url, principal_id
        ))
        .bearer_auth(admin_token)
        .json(&serde_json::json!({"description": "Test token"}))
        .send()
        .await
        .expect("create principal token")
        .json()
        .await
        .expect("parse token response");
    let principal_token = resp["data"]["token"]
        .as_str()
        .expect("principal token")
        .to_string();
    let token_id = resp["data"]["metadata"]["id"]
        .as_str()
        .expect("token id")
        .to_string();

    let resp: Value = client
        .post(format!("{}/api/v1/repos", base_url))
        .bearer_auth(&principal_token)
        .json(&serde_json::json!({
            "name": "test-repo",
            "description": "Test repository",
            "namespace": "test-principal"
        }))
        .send()
        .await
        .expect("create repo")
        .json()
        .await
        .expect("parse repo response");
    let repo_id = resp["data"]["id"].as_str().expect("repo id").to_string();

    let commit_sha = create_test_git_content(data_dir, &principal_ns_id);
    let git_auth =
        base64::engine::general_purpose::STANDARD.encode(format!("x-token:{}", principal_token));
    let test_suffix = chrono::Utc::now().timestamp().to_string();

    TestData {
        namespace_id,
        principal_id,
        principal_ns_id,
        principal_token,
        token_id,
        repo_id,
        commit_sha,
        git_auth_header: format!("Basic {}", git_auth),
        test_suffix,
    }
}

fn create_test_git_content(data_dir: &std::path::Path, namespace_id: &str) -> String {
    let repo_path = data_dir
        .join("repos")
        .join(namespace_id)
        .join("test-repo.git");
    std::fs::create_dir_all(&repo_path).expect("create repo dir");

    let repo = git2::Repository::init_bare(&repo_path).expect("init bare repo");

    let readme_content =
        b"# Test Repository\nThis is a test repository for API testing.\nMore content";
    let readme_oid = repo.blob(readme_content).expect("create readme blob");

    let main_content = b"fn main() { println!(\"Hello\"); }";
    let main_oid = repo.blob(main_content).expect("create main blob");

    let mut src_tree = repo.treebuilder(None).expect("create src treebuilder");
    src_tree
        .insert("main.rs", main_oid, 0o100644)
        .expect("insert main.rs");
    let src_tree_oid = src_tree.write().expect("write src tree");

    let mut root_tree = repo.treebuilder(None).expect("create root treebuilder");
    root_tree
        .insert("README.md", readme_oid, 0o100644)
        .expect("insert README.md");
    root_tree
        .insert("src", src_tree_oid, 0o040000)
        .expect("insert src");
    let root_tree_oid = root_tree.write().expect("write root tree");
    let tree = repo.find_tree(root_tree_oid).expect("find root tree");

    let sig = git2::Signature::now("Test User", "test@example.com").expect("create signature");
    let commit1 = repo
        .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .expect("create initial commit");

    let readme2 =
        b"# Test Repository\nThis is a test repository for API testing.\nMore content\nEven more";
    let readme2_oid = repo.blob(readme2).expect("create readme2 blob");
    let mut root_tree2 = repo.treebuilder(None).expect("create root treebuilder 2");
    root_tree2
        .insert("README.md", readme2_oid, 0o100644)
        .expect("insert README.md 2");
    root_tree2
        .insert("src", src_tree_oid, 0o040000)
        .expect("insert src 2");
    let root_tree2_oid = root_tree2.write().expect("write root tree 2");
    let tree2 = repo.find_tree(root_tree2_oid).expect("find root tree 2");
    let parent = repo.find_commit(commit1).expect("find parent commit");
    let commit2 = repo
        .commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Add more content",
            &tree2,
            &[&parent],
        )
        .expect("create second commit");

    commit2.to_string()
}

#[tokio::test]
async fn api_hurl_tests() {
    if Command::new("hurl").arg("--version").output().is_err() {
        eprintln!("Skipping API tests: hurl not found in PATH");
        eprintln!("Install: https://hurl.dev/docs/installation.html");
        return;
    }

    let server = common::TestServer::start().await;
    let test_data =
        create_test_data(&server.base_url, &server.admin_token, server.data_dir()).await;

    let vars_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/api/vars.env");
    test_data.write_vars_file(&vars_path, &server.base_url, &server.admin_token);

    let test_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/api");
    let test_files = [
        "health.hurl",
        "admin/grants.hurl",
        "admin/namespaces.hurl",
        "admin/tokens.hurl",
        "admin/principals.hurl",
        "user/namespaces.hurl",
        "user/repos.hurl",
        "user/repo_tags.hurl",
        "user/repo_folder.hurl",
        "user/tags.hurl",
        "user/folders.hurl",
        "content/refs.hurl",
        "content/commits.hurl",
        "content/tree.hurl",
        "content/blob.hurl",
        "content/mutations.hurl",
        "content/compare.hurl",
        "content/blame.hurl",
        "content/archive.hurl",
        "content/readme.hurl",
        "git/protocol.hurl",
        "lfs/batch.hurl",
        "lfs/objects.hurl",
    ];

    let test_paths: Vec<_> = test_files.iter().map(|f| test_dir.join(f)).collect();

    // --jobs 1 required because some tests share state (e.g., lfs/batch.hurl and lfs/objects.hurl)
    let status = std::process::Command::new("hurl")
        .arg("--test")
        .arg("--jobs")
        .arg("1")
        .arg("--connect-timeout")
        .arg("5")
        .arg("--variables-file")
        .arg(&vars_path)
        .args(&test_paths)
        .status()
        .expect("run hurl");

    let _ = std::fs::remove_file(&vars_path);

    assert!(status.success(), "hurl tests failed");
}
