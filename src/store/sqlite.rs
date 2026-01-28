use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use super::Store;
use super::schema::SCHEMA;
use crate::error::{Error, Result};
use crate::types::*;

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Handle SQLite's default datetime format: "YYYY-MM-DD HH:MM:SS"
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").map(|ndt| ndt.and_utc())
        })
        .unwrap_or_else(|e| {
            tracing::error!("Invalid datetime in database: '{}' - {}", s, e);
            Utc::now()
        })
}

fn format_datetime(dt: &DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

impl Store for SqliteStore {
    fn initialize(&self) -> Result<()> {
        self.conn().execute_batch(SCHEMA)?;
        Ok(())
    }

    // Namespace operations

    fn create_namespace(&self, ns: &Namespace) -> Result<()> {
        self.conn().execute(
            "INSERT INTO namespaces (id, name, created_at, repo_limit, storage_limit_bytes, external_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                ns.id,
                ns.name,
                format_datetime(&ns.created_at),
                ns.repo_limit,
                ns.storage_limit_bytes,
                ns.external_id,
            ],
        )?;
        Ok(())
    }

    fn get_namespace(&self, id: &str) -> Result<Option<Namespace>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, name, created_at, repo_limit, storage_limit_bytes, external_id
             FROM namespaces WHERE id = ?1",
            params![id],
            |row| {
                Ok(Namespace {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: parse_datetime(&row.get::<_, String>(2)?),
                    repo_limit: row.get(3)?,
                    storage_limit_bytes: row.get(4)?,
                    external_id: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn get_namespace_by_name(&self, name: &str) -> Result<Option<Namespace>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, name, created_at, repo_limit, storage_limit_bytes, external_id
             FROM namespaces WHERE name = ?1",
            params![name],
            |row| {
                Ok(Namespace {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: parse_datetime(&row.get::<_, String>(2)?),
                    repo_limit: row.get(3)?,
                    storage_limit_bytes: row.get(4)?,
                    external_id: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn list_namespaces(&self, cursor: &str, limit: i32) -> Result<Vec<Namespace>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, created_at, repo_limit, storage_limit_bytes, external_id
             FROM namespaces WHERE id > ?1 ORDER BY id LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![cursor, limit], |row| {
            Ok(Namespace {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: parse_datetime(&row.get::<_, String>(2)?),
                repo_limit: row.get(3)?,
                storage_limit_bytes: row.get(4)?,
                external_id: row.get(5)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn update_namespace(&self, ns: &Namespace) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE namespaces SET name = ?1, repo_limit = ?2, storage_limit_bytes = ?3 WHERE id = ?4",
            params![ns.name, ns.repo_limit, ns.storage_limit_bytes, ns.id],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn delete_namespace(&self, id: &str) -> Result<bool> {
        let rows = self
            .conn()
            .execute("DELETE FROM namespaces WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    // User operations

    fn create_user(&self, user: &User) -> Result<()> {
        self.conn().execute(
            "INSERT INTO users (id, primary_namespace_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                user.id,
                user.primary_namespace_id,
                format_datetime(&user.created_at),
                format_datetime(&user.updated_at),
            ],
        )?;
        Ok(())
    }

    fn get_user(&self, id: &str) -> Result<Option<User>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, primary_namespace_id, created_at, updated_at FROM users WHERE id = ?1",
            params![id],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    primary_namespace_id: row.get(1)?,
                    created_at: parse_datetime(&row.get::<_, String>(2)?),
                    updated_at: parse_datetime(&row.get::<_, String>(3)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn get_user_by_primary_namespace_id(&self, namespace_id: &str) -> Result<Option<User>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, primary_namespace_id, created_at, updated_at
             FROM users WHERE primary_namespace_id = ?1",
            params![namespace_id],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    primary_namespace_id: row.get(1)?,
                    created_at: parse_datetime(&row.get::<_, String>(2)?),
                    updated_at: parse_datetime(&row.get::<_, String>(3)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn list_users(&self, cursor: &str, limit: i32) -> Result<Vec<User>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, primary_namespace_id, created_at, updated_at
             FROM users WHERE id > ?1 ORDER BY id LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![cursor, limit], |row| {
            Ok(User {
                id: row.get(0)?,
                primary_namespace_id: row.get(1)?,
                created_at: parse_datetime(&row.get::<_, String>(2)?),
                updated_at: parse_datetime(&row.get::<_, String>(3)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn update_user(&self, user: &User) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE users SET primary_namespace_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                user.primary_namespace_id,
                format_datetime(&user.updated_at),
                user.id
            ],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn delete_user(&self, id: &str) -> Result<bool> {
        let rows = self
            .conn()
            .execute("DELETE FROM users WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    // Token operations

    fn create_token(&self, token: &Token) -> Result<()> {
        let result = self.conn().execute(
            "INSERT INTO tokens (id, token_hash, token_lookup, is_admin, user_id, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                token.id,
                token.token_hash,
                token.token_lookup,
                token.is_admin,
                token.user_id,
                format_datetime(&token.created_at),
                token.expires_at.as_ref().map(format_datetime),
            ],
        );

        match result {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(err, _))
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                Err(Error::TokenLookupCollision)
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    fn get_token_by_id(&self, id: &str) -> Result<Option<Token>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, token_hash, token_lookup, is_admin, user_id, created_at, expires_at, last_used_at
             FROM tokens WHERE id = ?1",
            params![id],
            |row| {
                Ok(Token {
                    id: row.get(0)?,
                    token_hash: row.get(1)?,
                    token_lookup: row.get(2)?,
                    is_admin: row.get(3)?,
                    user_id: row.get(4)?,
                    created_at: parse_datetime(&row.get::<_, String>(5)?),
                    expires_at: row.get::<_, Option<String>>(6)?.map(|s| parse_datetime(&s)),
                    last_used_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn get_token_by_lookup(&self, lookup: &str) -> Result<Option<Token>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, token_hash, token_lookup, is_admin, user_id, created_at, expires_at, last_used_at
             FROM tokens WHERE token_lookup = ?1",
            params![lookup],
            |row| {
                Ok(Token {
                    id: row.get(0)?,
                    token_hash: row.get(1)?,
                    token_lookup: row.get(2)?,
                    is_admin: row.get(3)?,
                    user_id: row.get(4)?,
                    created_at: parse_datetime(&row.get::<_, String>(5)?),
                    expires_at: row.get::<_, Option<String>>(6)?.map(|s| parse_datetime(&s)),
                    last_used_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn list_tokens(&self, cursor: &str, limit: i32) -> Result<Vec<Token>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, token_hash, token_lookup, is_admin, user_id, created_at, expires_at, last_used_at
             FROM tokens WHERE id > ?1 ORDER BY id LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![cursor, limit], |row| {
            Ok(Token {
                id: row.get(0)?,
                token_hash: row.get(1)?,
                token_lookup: row.get(2)?,
                is_admin: row.get(3)?,
                user_id: row.get(4)?,
                created_at: parse_datetime(&row.get::<_, String>(5)?),
                expires_at: row.get::<_, Option<String>>(6)?.map(|s| parse_datetime(&s)),
                last_used_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn list_user_tokens(&self, user_id: &str) -> Result<Vec<Token>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, token_hash, token_lookup, is_admin, user_id, created_at, expires_at, last_used_at
             FROM tokens WHERE user_id = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map(params![user_id], |row| {
            Ok(Token {
                id: row.get(0)?,
                token_hash: row.get(1)?,
                token_lookup: row.get(2)?,
                is_admin: row.get(3)?,
                user_id: row.get(4)?,
                created_at: parse_datetime(&row.get::<_, String>(5)?),
                expires_at: row.get::<_, Option<String>>(6)?.map(|s| parse_datetime(&s)),
                last_used_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn delete_token(&self, id: &str) -> Result<bool> {
        let rows = self
            .conn()
            .execute("DELETE FROM tokens WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    fn update_token_last_used(&self, id: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE tokens SET last_used_at = ?1 WHERE id = ?2",
            params![format_datetime(&Utc::now()), id],
        )?;
        Ok(())
    }

    // Repo operations

    fn create_repo(&self, repo: &Repo) -> Result<()> {
        self.conn().execute(
            "INSERT INTO repos (id, namespace_id, name, description, public, folder_id, size_bytes, last_push_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                repo.id,
                repo.namespace_id,
                repo.name,
                repo.description,
                repo.public,
                repo.folder_id,
                repo.size_bytes,
                repo.last_push_at.as_ref().map(format_datetime),
                format_datetime(&repo.created_at),
                format_datetime(&repo.updated_at),
            ],
        )?;
        Ok(())
    }

    fn get_repo(&self, namespace_id: &str, name: &str) -> Result<Option<Repo>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, namespace_id, name, description, public, folder_id, size_bytes, last_push_at, created_at, updated_at
             FROM repos WHERE namespace_id = ?1 AND name = ?2",
            params![namespace_id, name],
            |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    namespace_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    public: row.get(4)?,
                    folder_id: row.get(5)?,
                    size_bytes: row.get(6)?,
                    last_push_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                    created_at: parse_datetime(&row.get::<_, String>(8)?),
                    updated_at: parse_datetime(&row.get::<_, String>(9)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn get_repo_by_id(&self, id: &str) -> Result<Option<Repo>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, namespace_id, name, description, public, folder_id, size_bytes, last_push_at, created_at, updated_at
             FROM repos WHERE id = ?1",
            params![id],
            |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    namespace_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    public: row.get(4)?,
                    folder_id: row.get(5)?,
                    size_bytes: row.get(6)?,
                    last_push_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                    created_at: parse_datetime(&row.get::<_, String>(8)?),
                    updated_at: parse_datetime(&row.get::<_, String>(9)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn list_repos(&self, namespace_id: &str, cursor: &str, limit: i32) -> Result<Vec<Repo>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, namespace_id, name, description, public, folder_id, size_bytes, last_push_at, created_at, updated_at
             FROM repos WHERE namespace_id = ?1 AND name > ?2 ORDER BY name LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![namespace_id, cursor, limit], |row| {
            Ok(Repo {
                id: row.get(0)?,
                namespace_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                public: row.get(4)?,
                folder_id: row.get(5)?,
                size_bytes: row.get(6)?,
                last_push_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                created_at: parse_datetime(&row.get::<_, String>(8)?),
                updated_at: parse_datetime(&row.get::<_, String>(9)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn update_repo(&self, repo: &Repo) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE repos SET name = ?1, description = ?2, public = ?3, updated_at = ?4 WHERE id = ?5",
            params![
                repo.name,
                repo.description,
                repo.public,
                format_datetime(&Utc::now()),
                repo.id
            ],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn delete_repo(&self, id: &str) -> Result<bool> {
        let rows = self
            .conn()
            .execute("DELETE FROM repos WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    fn update_repo_last_push(&self, id: &str) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE repos SET last_push_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![format_datetime(&Utc::now()), id],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn update_repo_size(&self, id: &str, size_bytes: i64) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE repos SET size_bytes = ?1, updated_at = ?2 WHERE id = ?3",
            params![size_bytes, format_datetime(&Utc::now()), id],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn create_tag(&self, tag: &Tag) -> Result<()> {
        self.conn().execute(
            "INSERT INTO tags (id, namespace_id, name, color, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                tag.id,
                tag.namespace_id,
                tag.name,
                tag.color,
                format_datetime(&tag.created_at),
            ],
        )?;
        Ok(())
    }

    fn get_tag_by_id(&self, id: &str) -> Result<Option<Tag>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, namespace_id, name, color, created_at FROM tags WHERE id = ?1",
            params![id],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    namespace_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    created_at: parse_datetime(&row.get::<_, String>(4)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn get_tag_by_name(&self, namespace_id: &str, name: &str) -> Result<Option<Tag>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, namespace_id, name, color, created_at
             FROM tags WHERE namespace_id = ?1 AND name = ?2",
            params![namespace_id, name],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    namespace_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    created_at: parse_datetime(&row.get::<_, String>(4)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn list_tags(&self, namespace_id: &str, cursor: &str, limit: i32) -> Result<Vec<Tag>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, namespace_id, name, color, created_at
             FROM tags WHERE namespace_id = ?1 AND name > ?2 ORDER BY name LIMIT ?3",
        )?;

        let rows = stmt.query_map(params![namespace_id, cursor, limit], |row| {
            Ok(Tag {
                id: row.get(0)?,
                namespace_id: row.get(1)?,
                name: row.get(2)?,
                color: row.get(3)?,
                created_at: parse_datetime(&row.get::<_, String>(4)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn update_tag(&self, tag: &Tag) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE tags SET name = ?1, color = ?2 WHERE id = ?3",
            params![tag.name, tag.color, tag.id],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn delete_tag(&self, id: &str) -> Result<bool> {
        let rows = self
            .conn()
            .execute("DELETE FROM tags WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    fn count_tag_repos(&self, id: &str) -> Result<i32> {
        let conn = self.conn();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM repo_tags WHERE tag_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn add_repo_tag(&self, repo_id: &str, tag_id: &str) -> Result<()> {
        self.conn().execute(
            "INSERT OR IGNORE INTO repo_tags (repo_id, tag_id) VALUES (?1, ?2)",
            params![repo_id, tag_id],
        )?;
        Ok(())
    }

    fn remove_repo_tag(&self, repo_id: &str, tag_id: &str) -> Result<bool> {
        let rows = self.conn().execute(
            "DELETE FROM repo_tags WHERE repo_id = ?1 AND tag_id = ?2",
            params![repo_id, tag_id],
        )?;
        Ok(rows > 0)
    }

    fn list_repo_tags(&self, repo_id: &str) -> Result<Vec<Tag>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.namespace_id, t.name, t.color, t.created_at
             FROM tags t
             JOIN repo_tags rt ON t.id = rt.tag_id
             WHERE rt.repo_id = ?1
             ORDER BY t.name",
        )?;

        let rows = stmt.query_map(params![repo_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                namespace_id: row.get(1)?,
                name: row.get(2)?,
                color: row.get(3)?,
                created_at: parse_datetime(&row.get::<_, String>(4)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn list_tag_repos(&self, tag_id: &str) -> Result<Vec<Repo>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT r.id, r.namespace_id, r.name, r.description, r.public, r.folder_id, r.size_bytes, r.last_push_at, r.created_at, r.updated_at
             FROM repos r
             JOIN repo_tags rt ON r.id = rt.repo_id
             WHERE rt.tag_id = ?1
             ORDER BY r.name",
        )?;

        let rows = stmt.query_map(params![tag_id], |row| {
            Ok(Repo {
                id: row.get(0)?,
                namespace_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                public: row.get(4)?,
                folder_id: row.get(5)?,
                size_bytes: row.get(6)?,
                last_push_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                created_at: parse_datetime(&row.get::<_, String>(8)?),
                updated_at: parse_datetime(&row.get::<_, String>(9)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn set_repo_tags(&self, repo_id: &str, tag_ids: &[String]) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;

        tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1", params![repo_id])?;

        for tag_id in tag_ids {
            tx.execute(
                "INSERT INTO repo_tags (repo_id, tag_id) VALUES (?1, ?2)",
                params![repo_id, tag_id],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn create_folder(&self, folder: &Folder) -> Result<()> {
        self.conn().execute(
            "INSERT INTO folders (id, namespace_id, parent_id, name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                folder.id,
                folder.namespace_id,
                folder.parent_id,
                folder.name,
                format_datetime(&folder.created_at),
                format_datetime(&folder.updated_at),
            ],
        )?;
        Ok(())
    }

    fn get_folder_by_id(&self, id: &str) -> Result<Option<Folder>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT id, namespace_id, parent_id, name, created_at, updated_at FROM folders WHERE id = ?1",
            params![id],
            |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    namespace_id: row.get(1)?,
                    parent_id: row.get(2)?,
                    name: row.get(3)?,
                    created_at: parse_datetime(&row.get::<_, String>(4)?),
                    updated_at: parse_datetime(&row.get::<_, String>(5)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn get_folder_by_name(
        &self,
        namespace_id: &str,
        parent_id: Option<&str>,
        name: &str,
    ) -> Result<Option<Folder>> {
        let conn = self.conn();
        let result = match parent_id {
            Some(pid) => conn.query_row(
                "SELECT id, namespace_id, parent_id, name, created_at, updated_at
                 FROM folders WHERE namespace_id = ?1 AND parent_id = ?2 AND name = ?3",
                params![namespace_id, pid, name],
                |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        namespace_id: row.get(1)?,
                        parent_id: row.get(2)?,
                        name: row.get(3)?,
                        created_at: parse_datetime(&row.get::<_, String>(4)?),
                        updated_at: parse_datetime(&row.get::<_, String>(5)?),
                    })
                },
            ),
            None => conn.query_row(
                "SELECT id, namespace_id, parent_id, name, created_at, updated_at
                 FROM folders WHERE namespace_id = ?1 AND parent_id IS NULL AND name = ?2",
                params![namespace_id, name],
                |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        namespace_id: row.get(1)?,
                        parent_id: row.get(2)?,
                        name: row.get(3)?,
                        created_at: parse_datetime(&row.get::<_, String>(4)?),
                        updated_at: parse_datetime(&row.get::<_, String>(5)?),
                    })
                },
            ),
        };
        result.optional().map_err(Error::from)
    }

    fn list_folders(
        &self,
        namespace_id: &str,
        parent_id: Option<&str>,
        cursor: &str,
        limit: i32,
    ) -> Result<Vec<Folder>> {
        let conn = self.conn();
        let mut folders = Vec::new();

        match parent_id {
            Some(pid) => {
                let mut stmt = conn.prepare(
                    "SELECT id, namespace_id, parent_id, name, created_at, updated_at
                     FROM folders WHERE namespace_id = ?1 AND parent_id = ?2 AND name > ?3 ORDER BY name LIMIT ?4",
                )?;
                let rows = stmt.query_map(params![namespace_id, pid, cursor, limit], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        namespace_id: row.get(1)?,
                        parent_id: row.get(2)?,
                        name: row.get(3)?,
                        created_at: parse_datetime(&row.get::<_, String>(4)?),
                        updated_at: parse_datetime(&row.get::<_, String>(5)?),
                    })
                })?;
                for row in rows {
                    folders.push(row?);
                }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, namespace_id, parent_id, name, created_at, updated_at
                     FROM folders WHERE namespace_id = ?1 AND parent_id IS NULL AND name > ?2 ORDER BY name LIMIT ?3",
                )?;
                let rows = stmt.query_map(params![namespace_id, cursor, limit], |row| {
                    Ok(Folder {
                        id: row.get(0)?,
                        namespace_id: row.get(1)?,
                        parent_id: row.get(2)?,
                        name: row.get(3)?,
                        created_at: parse_datetime(&row.get::<_, String>(4)?),
                        updated_at: parse_datetime(&row.get::<_, String>(5)?),
                    })
                })?;
                for row in rows {
                    folders.push(row?);
                }
            }
        }

        Ok(folders)
    }

    fn list_folder_children(&self, folder_id: &str) -> Result<Vec<Folder>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, namespace_id, parent_id, name, created_at, updated_at
             FROM folders WHERE parent_id = ?1 ORDER BY name",
        )?;

        let rows = stmt.query_map(params![folder_id], |row| {
            Ok(Folder {
                id: row.get(0)?,
                namespace_id: row.get(1)?,
                parent_id: row.get(2)?,
                name: row.get(3)?,
                created_at: parse_datetime(&row.get::<_, String>(4)?),
                updated_at: parse_datetime(&row.get::<_, String>(5)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn list_folder_ancestors(&self, folder_id: &str) -> Result<Vec<Folder>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "WITH RECURSIVE ancestors AS (
                SELECT id, namespace_id, parent_id, name, created_at, updated_at, 0 as depth
                FROM folders WHERE id = ?1
                UNION ALL
                SELECT f.id, f.namespace_id, f.parent_id, f.name, f.created_at, f.updated_at, a.depth + 1
                FROM folders f
                JOIN ancestors a ON f.id = a.parent_id
            )
            SELECT id, namespace_id, parent_id, name, created_at, updated_at
            FROM ancestors WHERE depth > 0 ORDER BY depth DESC",
        )?;

        let rows = stmt.query_map(params![folder_id], |row| {
            Ok(Folder {
                id: row.get(0)?,
                namespace_id: row.get(1)?,
                parent_id: row.get(2)?,
                name: row.get(3)?,
                created_at: parse_datetime(&row.get::<_, String>(4)?),
                updated_at: parse_datetime(&row.get::<_, String>(5)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn update_folder(&self, folder: &Folder) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE folders SET name = ?1, parent_id = ?2, updated_at = ?3 WHERE id = ?4",
            params![
                folder.name,
                folder.parent_id,
                format_datetime(&Utc::now()),
                folder.id
            ],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn move_folder(&self, folder_id: &str, new_parent_id: Option<&str>) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE folders SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_parent_id, format_datetime(&Utc::now()), folder_id],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn delete_folder(&self, id: &str, recursive: bool) -> Result<bool> {
        if recursive {
            let rows = self.conn().execute(
                "WITH RECURSIVE descendants AS (
                    SELECT id FROM folders WHERE id = ?1
                    UNION ALL
                    SELECT f.id FROM folders f JOIN descendants d ON f.parent_id = d.id
                )
                DELETE FROM folders WHERE id IN (SELECT id FROM descendants)",
                params![id],
            )?;
            Ok(rows > 0)
        } else {
            let rows = self
                .conn()
                .execute("DELETE FROM folders WHERE id = ?1", params![id])?;
            Ok(rows > 0)
        }
    }

    fn get_folder_path(&self, folder_id: &str) -> Result<String> {
        let ancestors = self.list_folder_ancestors(folder_id)?;
        let folder = self.get_folder_by_id(folder_id)?;

        match folder {
            Some(f) => {
                let mut path_parts: Vec<&str> = ancestors.iter().map(|a| a.name.as_str()).collect();
                path_parts.push(&f.name);
                Ok(path_parts.join("/"))
            }
            None => Err(Error::NotFound),
        }
    }

    fn count_folder_repos(&self, folder_id: &str, recursive: bool) -> Result<i32> {
        let conn = self.conn();
        let count: i32 = if recursive {
            conn.query_row(
                "WITH RECURSIVE descendants AS (
                    SELECT id FROM folders WHERE id = ?1
                    UNION ALL
                    SELECT f.id FROM folders f JOIN descendants d ON f.parent_id = d.id
                )
                SELECT COUNT(*) FROM repos WHERE folder_id IN (SELECT id FROM descendants)",
                params![folder_id],
                |row| row.get(0),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM repos WHERE folder_id = ?1",
                params![folder_id],
                |row| row.get(0),
            )?
        };
        Ok(count)
    }

    fn set_repo_folder(&self, repo_id: &str, folder_id: Option<&str>) -> Result<()> {
        let rows = self.conn().execute(
            "UPDATE repos SET folder_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![folder_id, format_datetime(&Utc::now()), repo_id],
        )?;

        if rows == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    fn list_folder_repos(&self, folder_id: &str, recursive: bool) -> Result<Vec<Repo>> {
        let conn = self.conn();

        if recursive {
            let mut stmt = conn.prepare(
                "WITH RECURSIVE descendants AS (
                    SELECT id FROM folders WHERE id = ?1
                    UNION ALL
                    SELECT f.id FROM folders f JOIN descendants d ON f.parent_id = d.id
                )
                SELECT r.id, r.namespace_id, r.name, r.description, r.public, r.folder_id, r.size_bytes, r.last_push_at, r.created_at, r.updated_at
                FROM repos r WHERE r.folder_id IN (SELECT id FROM descendants) ORDER BY r.name",
            )?;

            let rows = stmt.query_map(params![folder_id], |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    namespace_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    public: row.get(4)?,
                    folder_id: row.get(5)?,
                    size_bytes: row.get(6)?,
                    last_push_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                    created_at: parse_datetime(&row.get::<_, String>(8)?),
                    updated_at: parse_datetime(&row.get::<_, String>(9)?),
                })
            })?;

            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Error::from)
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, namespace_id, name, description, public, folder_id, size_bytes, last_push_at, created_at, updated_at
                 FROM repos WHERE folder_id = ?1 ORDER BY name",
            )?;

            let rows = stmt.query_map(params![folder_id], |row| {
                Ok(Repo {
                    id: row.get(0)?,
                    namespace_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    public: row.get(4)?,
                    folder_id: row.get(5)?,
                    size_bytes: row.get(6)?,
                    last_push_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                    created_at: parse_datetime(&row.get::<_, String>(8)?),
                    updated_at: parse_datetime(&row.get::<_, String>(9)?),
                })
            })?;

            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Error::from)
        }
    }

    // Namespace grant operations

    fn upsert_namespace_grant(&self, grant: &NamespaceGrant) -> Result<()> {
        // Check if the namespace belongs to another user as their primary
        if let Some(owner) = self.get_user_by_primary_namespace_id(&grant.namespace_id)? {
            if owner.id != grant.user_id {
                return Err(Error::PrimaryNamespaceGrant);
            }
        }

        self.conn().execute(
            "INSERT INTO user_namespace_grants (user_id, namespace_id, allow_bits, deny_bits, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (user_id, namespace_id) DO UPDATE SET
                allow_bits = excluded.allow_bits,
                deny_bits = excluded.deny_bits,
                updated_at = excluded.updated_at",
            params![
                grant.user_id,
                grant.namespace_id,
                i64::from(grant.allow_bits),
                i64::from(grant.deny_bits),
                format_datetime(&grant.created_at),
                format_datetime(&grant.updated_at),
            ],
        )?;
        Ok(())
    }

    fn delete_namespace_grant(&self, user_id: &str, namespace_id: &str) -> Result<bool> {
        let rows = self.conn().execute(
            "DELETE FROM user_namespace_grants WHERE user_id = ?1 AND namespace_id = ?2",
            params![user_id, namespace_id],
        )?;
        Ok(rows > 0)
    }

    fn get_namespace_grant(
        &self,
        user_id: &str,
        namespace_id: &str,
    ) -> Result<Option<NamespaceGrant>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT user_id, namespace_id, allow_bits, deny_bits, created_at, updated_at
             FROM user_namespace_grants WHERE user_id = ?1 AND namespace_id = ?2",
            params![user_id, namespace_id],
            |row| {
                Ok(NamespaceGrant {
                    user_id: row.get(0)?,
                    namespace_id: row.get(1)?,
                    allow_bits: Permission::from(row.get::<_, i64>(2)?),
                    deny_bits: Permission::from(row.get::<_, i64>(3)?),
                    created_at: parse_datetime(&row.get::<_, String>(4)?),
                    updated_at: parse_datetime(&row.get::<_, String>(5)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn list_user_namespace_grants(&self, user_id: &str) -> Result<Vec<NamespaceGrant>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT user_id, namespace_id, allow_bits, deny_bits, created_at, updated_at
             FROM user_namespace_grants WHERE user_id = ?1 ORDER BY namespace_id",
        )?;

        let rows = stmt.query_map(params![user_id], |row| {
            Ok(NamespaceGrant {
                user_id: row.get(0)?,
                namespace_id: row.get(1)?,
                allow_bits: Permission::from(row.get::<_, i64>(2)?),
                deny_bits: Permission::from(row.get::<_, i64>(3)?),
                created_at: parse_datetime(&row.get::<_, String>(4)?),
                updated_at: parse_datetime(&row.get::<_, String>(5)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn list_namespace_grants_for_namespace(
        &self,
        namespace_id: &str,
    ) -> Result<Vec<NamespaceGrant>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT user_id, namespace_id, allow_bits, deny_bits, created_at, updated_at
             FROM user_namespace_grants WHERE namespace_id = ?1 ORDER BY user_id",
        )?;

        let rows = stmt.query_map(params![namespace_id], |row| {
            Ok(NamespaceGrant {
                user_id: row.get(0)?,
                namespace_id: row.get(1)?,
                allow_bits: Permission::from(row.get::<_, i64>(2)?),
                deny_bits: Permission::from(row.get::<_, i64>(3)?),
                created_at: parse_datetime(&row.get::<_, String>(4)?),
                updated_at: parse_datetime(&row.get::<_, String>(5)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn count_namespace_users(&self, namespace_id: &str) -> Result<i32> {
        let conn = self.conn();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM user_namespace_grants WHERE namespace_id = ?1",
            params![namespace_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // Repo grant operations

    fn upsert_repo_grant(&self, grant: &RepoGrant) -> Result<()> {
        self.conn().execute(
            "INSERT INTO user_repo_grants (user_id, repo_id, allow_bits, deny_bits, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (user_id, repo_id) DO UPDATE SET
                allow_bits = excluded.allow_bits,
                deny_bits = excluded.deny_bits,
                updated_at = excluded.updated_at",
            params![
                grant.user_id,
                grant.repo_id,
                i64::from(grant.allow_bits),
                i64::from(grant.deny_bits),
                format_datetime(&grant.created_at),
                format_datetime(&grant.updated_at),
            ],
        )?;
        Ok(())
    }

    fn delete_repo_grant(&self, user_id: &str, repo_id: &str) -> Result<bool> {
        let rows = self.conn().execute(
            "DELETE FROM user_repo_grants WHERE user_id = ?1 AND repo_id = ?2",
            params![user_id, repo_id],
        )?;
        Ok(rows > 0)
    }

    fn get_repo_grant(&self, user_id: &str, repo_id: &str) -> Result<Option<RepoGrant>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT user_id, repo_id, allow_bits, deny_bits, created_at, updated_at
             FROM user_repo_grants WHERE user_id = ?1 AND repo_id = ?2",
            params![user_id, repo_id],
            |row| {
                Ok(RepoGrant {
                    user_id: row.get(0)?,
                    repo_id: row.get(1)?,
                    allow_bits: Permission::from(row.get::<_, i64>(2)?),
                    deny_bits: Permission::from(row.get::<_, i64>(3)?),
                    created_at: parse_datetime(&row.get::<_, String>(4)?),
                    updated_at: parse_datetime(&row.get::<_, String>(5)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn list_user_repo_grants(&self, user_id: &str) -> Result<Vec<RepoGrant>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT user_id, repo_id, allow_bits, deny_bits, created_at, updated_at
             FROM user_repo_grants WHERE user_id = ?1 ORDER BY repo_id",
        )?;

        let rows = stmt.query_map(params![user_id], |row| {
            Ok(RepoGrant {
                user_id: row.get(0)?,
                repo_id: row.get(1)?,
                allow_bits: Permission::from(row.get::<_, i64>(2)?),
                deny_bits: Permission::from(row.get::<_, i64>(3)?),
                created_at: parse_datetime(&row.get::<_, String>(4)?),
                updated_at: parse_datetime(&row.get::<_, String>(5)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn list_user_repos_with_grants(&self, user_id: &str, namespace_id: &str) -> Result<Vec<Repo>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT r.id, r.namespace_id, r.name, r.description, r.public, r.folder_id, r.size_bytes, r.last_push_at, r.created_at, r.updated_at
             FROM repos r
             JOIN user_repo_grants g ON r.id = g.repo_id
             WHERE g.user_id = ?1 AND r.namespace_id = ?2
             ORDER BY r.name",
        )?;

        let rows = stmt.query_map(params![user_id, namespace_id], |row| {
            Ok(Repo {
                id: row.get(0)?,
                namespace_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                public: row.get(4)?,
                folder_id: row.get(5)?,
                size_bytes: row.get(6)?,
                last_push_at: row.get::<_, Option<String>>(7)?.map(|s| parse_datetime(&s)),
                created_at: parse_datetime(&row.get::<_, String>(8)?),
                updated_at: parse_datetime(&row.get::<_, String>(9)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn has_repo_grants_in_namespace(&self, user_id: &str, namespace_id: &str) -> Result<bool> {
        let conn = self.conn();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM user_repo_grants g
             JOIN repos r ON r.id = g.repo_id
             WHERE g.user_id = ?1 AND r.namespace_id = ?2",
            params![user_id, namespace_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // LFS object operations

    fn create_lfs_object(&self, obj: &LfsObject) -> Result<()> {
        self.conn().execute(
            "INSERT INTO lfs_objects (repo_id, oid, size, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                obj.repo_id,
                obj.oid,
                obj.size,
                format_datetime(&obj.created_at),
            ],
        )?;
        Ok(())
    }

    fn get_lfs_object(&self, repo_id: &str, oid: &str) -> Result<Option<LfsObject>> {
        let conn = self.conn();
        conn.query_row(
            "SELECT repo_id, oid, size, created_at FROM lfs_objects WHERE repo_id = ?1 AND oid = ?2",
            params![repo_id, oid],
            |row| {
                Ok(LfsObject {
                    repo_id: row.get(0)?,
                    oid: row.get(1)?,
                    size: row.get(2)?,
                    created_at: parse_datetime(&row.get::<_, String>(3)?),
                })
            },
        )
        .optional()
        .map_err(Error::from)
    }

    fn list_lfs_objects(&self, repo_id: &str) -> Result<Vec<LfsObject>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT repo_id, oid, size, created_at FROM lfs_objects WHERE repo_id = ?1 ORDER BY created_at",
        )?;

        let rows = stmt.query_map(params![repo_id], |row| {
            Ok(LfsObject {
                repo_id: row.get(0)?,
                oid: row.get(1)?,
                size: row.get(2)?,
                created_at: parse_datetime(&row.get::<_, String>(3)?),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    fn delete_lfs_object(&self, repo_id: &str, oid: &str) -> Result<bool> {
        let rows = self.conn().execute(
            "DELETE FROM lfs_objects WHERE repo_id = ?1 AND oid = ?2",
            params![repo_id, oid],
        )?;
        Ok(rows > 0)
    }

    fn get_repo_lfs_size(&self, repo_id: &str) -> Result<i64> {
        let conn = self.conn();
        let size: Option<i64> = conn
            .query_row(
                "SELECT SUM(size) FROM lfs_objects WHERE repo_id = ?1",
                params![repo_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(size.unwrap_or(0))
    }

    fn has_admin_token(&self) -> Result<bool> {
        let conn = self.conn();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tokens WHERE is_admin = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn close(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_initialize_creates_tables() {
        let temp = TempDir::new().unwrap();
        let store = SqliteStore::new(temp.path().join("test.db")).unwrap();
        store.initialize().unwrap();

        let conn = store.conn();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"namespaces".to_string()));
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"tokens".to_string()));
        assert!(tables.contains(&"repos".to_string()));
        assert!(tables.contains(&"tags".to_string()));
        assert!(tables.contains(&"repo_tags".to_string()));
        assert!(tables.contains(&"folders".to_string()));
        assert!(tables.contains(&"user_namespace_grants".to_string()));
        assert!(tables.contains(&"user_repo_grants".to_string()));
        assert!(tables.contains(&"lfs_objects".to_string()));
    }

    #[test]
    fn test_namespace_crud() {
        let temp = TempDir::new().unwrap();
        let store = SqliteStore::new(temp.path().join("test.db")).unwrap();
        store.initialize().unwrap();

        let ns = Namespace {
            id: "ns-1".to_string(),
            name: "test-namespace".to_string(),
            created_at: Utc::now(),
            repo_limit: Some(10),
            storage_limit_bytes: Some(1024 * 1024),
            external_id: None,
        };

        store.create_namespace(&ns).unwrap();

        let fetched = store.get_namespace("ns-1").unwrap().unwrap();
        assert_eq!(fetched.name, "test-namespace");
        assert_eq!(fetched.repo_limit, Some(10));

        let by_name = store
            .get_namespace_by_name("test-namespace")
            .unwrap()
            .unwrap();
        assert_eq!(by_name.id, "ns-1");

        let deleted = store.delete_namespace("ns-1").unwrap();
        assert!(deleted);

        let gone = store.get_namespace("ns-1").unwrap();
        assert!(gone.is_none());
    }

    #[test]
    fn test_user_crud() {
        let temp = TempDir::new().unwrap();
        let store = SqliteStore::new(temp.path().join("test.db")).unwrap();
        store.initialize().unwrap();

        let ns = Namespace {
            id: "ns-1".to_string(),
            name: "test-ns".to_string(),
            created_at: Utc::now(),
            repo_limit: None,
            storage_limit_bytes: None,
            external_id: None,
        };
        store.create_namespace(&ns).unwrap();

        let user = User {
            id: "user-1".to_string(),
            primary_namespace_id: "ns-1".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        store.create_user(&user).unwrap();

        let fetched = store.get_user("user-1").unwrap().unwrap();
        assert_eq!(fetched.primary_namespace_id, "ns-1");

        let by_ns = store
            .get_user_by_primary_namespace_id("ns-1")
            .unwrap()
            .unwrap();
        assert_eq!(by_ns.id, "user-1");
    }

    #[test]
    fn test_token_lookup_collision() {
        let temp = TempDir::new().unwrap();
        let store = SqliteStore::new(temp.path().join("test.db")).unwrap();
        store.initialize().unwrap();

        let token1 = Token {
            id: "token-1".to_string(),
            token_hash: "hash1".to_string(),
            token_lookup: "lookup123".to_string(),
            is_admin: true,
            user_id: None,
            created_at: Utc::now(),
            expires_at: None,
            last_used_at: None,
        };
        store.create_token(&token1).unwrap();

        let token2 = Token {
            id: "token-2".to_string(),
            token_hash: "hash2".to_string(),
            token_lookup: "lookup123".to_string(), // Same lookup
            is_admin: true,
            user_id: None,
            created_at: Utc::now(),
            expires_at: None,
            last_used_at: None,
        };

        let result = store.create_token(&token2);
        assert!(matches!(result, Err(Error::TokenLookupCollision)));
    }
}
