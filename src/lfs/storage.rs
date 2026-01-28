use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::fs::{self, File};
use tokio::io::{AsyncWriteExt, BufReader};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LfsStorageError {
    #[error("object not found")]
    NotFound,
    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("invalid OID format")]
    InvalidOid,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl LfsStorageError {
    fn from_io(e: std::io::Error) -> Self {
        if e.kind() == ErrorKind::NotFound {
            Self::NotFound
        } else {
            Self::Io(e)
        }
    }
}

pub struct LfsStorage {
    base_path: PathBuf,
}

impl LfsStorage {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            base_path: data_dir.join("lfs"),
        }
    }

    fn object_path(&self, repo_id: &str, oid: &str) -> PathBuf {
        let prefix1 = &oid[0..2];
        let prefix2 = &oid[2..4];
        self.base_path
            .join(repo_id)
            .join("objects")
            .join(prefix1)
            .join(prefix2)
            .join(oid)
    }

    fn temp_path(&self, repo_id: &str) -> PathBuf {
        self.base_path
            .join(repo_id)
            .join("tmp")
            .join(Uuid::new_v4().to_string())
    }

    pub async fn exists(&self, repo_id: &str, oid: &str) -> Result<bool, LfsStorageError> {
        validate_oid(oid)?;
        let path = self.object_path(repo_id, oid);
        Ok(path.exists())
    }

    pub async fn size(&self, repo_id: &str, oid: &str) -> Result<i64, LfsStorageError> {
        validate_oid(oid)?;
        let path = self.object_path(repo_id, oid);
        let metadata = fs::metadata(&path)
            .await
            .map_err(LfsStorageError::from_io)?;
        Ok(metadata.len() as i64)
    }

    pub async fn get(
        &self,
        repo_id: &str,
        oid: &str,
    ) -> Result<(BufReader<File>, i64), LfsStorageError> {
        validate_oid(oid)?;
        let path = self.object_path(repo_id, oid);
        let file = File::open(&path).await.map_err(LfsStorageError::from_io)?;

        let metadata = file.metadata().await?;
        let size = metadata.len() as i64;

        Ok((BufReader::new(file), size))
    }

    pub async fn put(
        &self,
        repo_id: &str,
        oid: &str,
        data: &[u8],
        expected_size: i64,
    ) -> Result<(), LfsStorageError> {
        validate_oid(oid)?;

        if data.len() as i64 != expected_size {
            return Err(LfsStorageError::HashMismatch {
                expected: format!("size {expected_size}"),
                actual: format!("size {}", data.len()),
            });
        }

        let mut hasher = Sha256::new();
        hasher.update(data);
        let actual_hash = hex::encode(hasher.finalize());

        if actual_hash != oid {
            return Err(LfsStorageError::HashMismatch {
                expected: oid.to_string(),
                actual: actual_hash,
            });
        }

        let temp_path = self.temp_path(repo_id);
        if let Some(parent) = temp_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut temp_file = File::create(&temp_path).await?;
        temp_file.write_all(data).await?;
        temp_file.sync_all().await?;

        let final_path = self.object_path(repo_id, oid);
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::rename(&temp_path, &final_path).await?;

        Ok(())
    }

    pub async fn delete(&self, repo_id: &str, oid: &str) -> Result<bool, LfsStorageError> {
        validate_oid(oid)?;
        let path = self.object_path(repo_id, oid);

        match fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(LfsStorageError::Io(e)),
        }
    }
}

fn validate_oid(oid: &str) -> Result<(), LfsStorageError> {
    if oid.len() != 64 {
        return Err(LfsStorageError::InvalidOid);
    }

    if !oid
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase())
    {
        return Err(LfsStorageError::InvalidOid);
    }

    Ok(())
}

#[must_use]
pub fn is_valid_oid(oid: &str) -> bool {
    validate_oid(oid).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncReadExt;

    fn test_oid() -> String {
        "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3".to_string()
    }

    fn test_data() -> Vec<u8> {
        b"123".to_vec()
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LfsStorage::new(temp_dir.path());

        let oid = test_oid();
        let data = test_data();
        let repo_id = "test-repo";

        storage
            .put(repo_id, &oid, &data, data.len() as i64)
            .await
            .unwrap();

        assert!(storage.exists(repo_id, &oid).await.unwrap());
        assert_eq!(
            storage.size(repo_id, &oid).await.unwrap(),
            data.len() as i64
        );

        let (mut reader, size) = storage.get(repo_id, &oid).await.unwrap();
        assert_eq!(size, data.len() as i64);

        let mut content = Vec::new();
        reader.read_to_end(&mut content).await.unwrap();
        assert_eq!(content, data);
    }

    #[tokio::test]
    async fn test_hash_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LfsStorage::new(temp_dir.path());

        let wrong_oid = "0000000000000000000000000000000000000000000000000000000000000000";
        let data = test_data();
        let repo_id = "test-repo";

        let result = storage
            .put(repo_id, wrong_oid, &data, data.len() as i64)
            .await;
        assert!(matches!(result, Err(LfsStorageError::HashMismatch { .. })));
    }

    #[tokio::test]
    async fn test_invalid_oid() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LfsStorage::new(temp_dir.path());

        assert!(matches!(
            storage.exists("repo", "invalid").await,
            Err(LfsStorageError::InvalidOid)
        ));

        assert!(matches!(
            storage
                .exists(
                    "repo",
                    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                )
                .await,
            Err(LfsStorageError::InvalidOid)
        ));
    }

    #[tokio::test]
    async fn test_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LfsStorage::new(temp_dir.path());

        let oid = test_oid();
        assert!(!storage.exists("repo", &oid).await.unwrap());
        assert!(matches!(
            storage.get("repo", &oid).await,
            Err(LfsStorageError::NotFound)
        ));
    }

    #[tokio::test]
    async fn test_delete() {
        let temp_dir = TempDir::new().unwrap();
        let storage = LfsStorage::new(temp_dir.path());

        let oid = test_oid();
        let data = test_data();
        let repo_id = "test-repo";

        storage
            .put(repo_id, &oid, &data, data.len() as i64)
            .await
            .unwrap();
        assert!(storage.delete(repo_id, &oid).await.unwrap());
        assert!(!storage.exists(repo_id, &oid).await.unwrap());
        assert!(!storage.delete(repo_id, &oid).await.unwrap());
    }

    #[test]
    fn test_is_valid_oid() {
        assert!(is_valid_oid(
            "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
        ));
        assert!(!is_valid_oid("short"));
        assert!(!is_valid_oid(
            "A665A45920422F9D417E4867EFDC4FB8A04A1F3FFF1FA07E998E86F7F7A27AE3"
        ));
        assert!(!is_valid_oid(
            "g665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
        ));
    }
}
