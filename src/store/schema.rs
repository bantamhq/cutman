pub const SCHEMA: &str = r#"
-- Namespaces provide isolation
CREATE TABLE IF NOT EXISTS namespaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TEXT DEFAULT (datetime('now')),

    -- Soft limits (enforced by platform, tracked by core)
    repo_limit INTEGER,           -- NULL = unlimited
    storage_limit_bytes INTEGER,  -- NULL = unlimited

    -- For platform correlation (opaque to core)
    external_id TEXT
);

-- Users own permissions; tokens are just auth credentials for users
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    primary_namespace_id TEXT NOT NULL UNIQUE REFERENCES namespaces(id) ON DELETE CASCADE,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

-- Repositories
CREATE TABLE IF NOT EXISTS repos (
    id TEXT PRIMARY KEY,
    namespace_id TEXT NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,

    -- Visibility
    public INTEGER DEFAULT 0,  -- If 1, anonymous read access allowed

    -- Folder assignment (one-to-many, repo belongs to one folder)
    folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL,

    -- Stats
    size_bytes INTEGER DEFAULT 0,
    last_push_at TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),

    UNIQUE(namespace_id, name)
);

-- Namespace grants: permissions a user has for a namespace
CREATE TABLE IF NOT EXISTS user_namespace_grants (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    namespace_id TEXT NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    allow_bits INTEGER NOT NULL DEFAULT 0,
    deny_bits INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, namespace_id)
);

-- Repo grants: permissions a user has for a specific repo
CREATE TABLE IF NOT EXISTS user_repo_grants (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    allow_bits INTEGER NOT NULL DEFAULT 0,
    deny_bits INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, repo_id)
);

-- Tokens are auth credentials; non-admin tokens must belong to a user
CREATE TABLE IF NOT EXISTS tokens (
    id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL,          -- argon2id hash with embedded salt
    token_lookup TEXT NOT NULL,        -- first 8 chars of ID for fast lookup
    is_admin INTEGER NOT NULL DEFAULT 0,  -- admin tokens only access /api/v1/admin/* routes

    -- User binding (required for non-admin tokens, NULL only for admin tokens)
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,

    -- Lifecycle
    created_at TEXT DEFAULT (datetime('now')),
    expires_at TEXT,            -- NULL = never
    last_used_at TEXT
);

-- Tags for labeling repos (many-to-many)
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    namespace_id TEXT NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT,
    created_at TEXT DEFAULT (datetime('now')),

    UNIQUE(namespace_id, name)
);

-- Many-to-many relationship between repos and tags
CREATE TABLE IF NOT EXISTS repo_tags (
    repo_id TEXT REFERENCES repos(id) ON DELETE CASCADE,
    tag_id TEXT REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (repo_id, tag_id)
);

-- Folders for organizing repos (hierarchical, one-to-many)
CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    namespace_id TEXT NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now')),

    UNIQUE(namespace_id, parent_id, name)
);

-- LFS objects
CREATE TABLE IF NOT EXISTS lfs_objects (
    repo_id TEXT NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
    oid TEXT NOT NULL,
    size INTEGER NOT NULL,
    created_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (repo_id, oid)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_repos_namespace ON repos(namespace_id);
CREATE INDEX IF NOT EXISTS idx_repos_folder ON repos(folder_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tokens_lookup ON tokens(token_lookup);
CREATE INDEX IF NOT EXISTS idx_tokens_user ON tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_tags_namespace ON tags(namespace_id);
CREATE INDEX IF NOT EXISTS idx_folders_namespace ON folders(namespace_id);
CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id);
CREATE INDEX IF NOT EXISTS idx_lfs_objects_repo ON lfs_objects(repo_id);
CREATE INDEX IF NOT EXISTS idx_namespace_grants_user ON user_namespace_grants(user_id);
CREATE INDEX IF NOT EXISTS idx_repo_grants_user ON user_repo_grants(user_id);
CREATE INDEX IF NOT EXISTS idx_users_primary_namespace ON users(primary_namespace_id);
"#;
