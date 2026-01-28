use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::LazyLock;

use tempfile::TempDir;

pub struct TestServer {
    pub temp_dir: TempDir,
    pub base_url: String,
    pub admin_token: String,
    server_process: Option<Child>,
}

static BUILD_RELEASE: LazyLock<()> = LazyLock::new(|| {
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("build release binary");
    assert!(build_status.success(), "Failed to build release binary");
});

impl TestServer {
    pub async fn start() -> Self {
        LazyLock::force(&BUILD_RELEASE);

        let temp_dir = TempDir::new().expect("create temp dir");
        let data_dir = temp_dir.path();
        let binary = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/release/cutman");

        let init_output = Command::new(&binary)
            .args(["admin", "init", "--data-dir"])
            .arg(data_dir)
            .arg("--non-interactive")
            .output()
            .expect("run init");
        assert!(
            init_output.status.success(),
            "Failed to initialize database"
        );

        let token_path = data_dir.join(".admin_token");
        let admin_token = std::fs::read_to_string(&token_path)
            .expect("read admin token")
            .trim()
            .to_string();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);

        let base_url = format!("http://127.0.0.1:{}", port);

        let server_process = Command::new(&binary)
            .args(["serve", "--data-dir"])
            .arg(data_dir)
            .args(["--host", "127.0.0.1", "--port"])
            .arg(port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start server");

        Self::wait_for_ready(&base_url).await;

        Self {
            temp_dir,
            base_url,
            admin_token,
            server_process: Some(server_process),
        }
    }

    async fn wait_for_ready(base_url: &str) {
        let client = reqwest::Client::new();
        for _ in 0..50 {
            if client
                .get(format!("{}/health", base_url))
                .send()
                .await
                .is_ok()
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("Server did not become ready");
    }

    pub fn data_dir(&self) -> &Path {
        self.temp_dir.path()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(mut process) = self.server_process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}
