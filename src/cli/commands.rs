use clap::Subcommand;

#[derive(Subcommand)]
pub enum AdminCommands {
    /// Initialize the server (create database and admin token)
    Init {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// Skip interactive prompts
        #[arg(long)]
        non_interactive: bool,
    },

    /// Manage users
    User {
        #[command(subcommand)]
        command: UserCommands,
    },

    /// Manage access tokens
    Token {
        #[command(subcommand)]
        command: TokenCommands,
    },

    /// Manage namespaces
    Namespace {
        #[command(subcommand)]
        command: NamespaceCommands,
    },

    /// Manage permissions
    Permission {
        #[command(subcommand)]
        command: PermissionCommands,
    },

    /// Show server status information
    Info {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum UserCommands {
    /// Add a new user with namespace and optional token
    Add {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// Username for the new user
        #[arg(long)]
        username: Option<String>,

        /// Create a token for the new user
        #[arg(long)]
        create_token: bool,

        /// Skip interactive prompts (requires --username)
        #[arg(long)]
        non_interactive: bool,

        /// List existing users instead of adding
        #[arg(long)]
        list: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Remove a user
    Remove {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// User ID to remove
        #[arg(long)]
        user_id: Option<String>,

        /// Skip interactive prompts (requires --user-id)
        #[arg(long)]
        non_interactive: bool,

        /// List users instead of removing
        #[arg(long)]
        list: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum TokenCommands {
    /// Create a new access token
    Create {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// User ID for the token
        #[arg(long)]
        user_id: Option<String>,

        /// Token expiration in days (omit for no expiration)
        #[arg(long)]
        expires_days: Option<i64>,

        /// Skip interactive prompts (requires --user-id)
        #[arg(long)]
        non_interactive: bool,

        /// List existing tokens instead of creating
        #[arg(long)]
        list: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Revoke an access token
    Revoke {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// Token ID to revoke
        #[arg(long)]
        token_id: Option<String>,

        /// Skip interactive prompts (requires --token-id)
        #[arg(long)]
        non_interactive: bool,

        /// List tokens instead of revoking
        #[arg(long)]
        list: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum NamespaceCommands {
    /// Add a new shared namespace
    Add {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// Name for the new namespace
        #[arg(long)]
        name: Option<String>,

        /// Skip interactive prompts (requires --name)
        #[arg(long)]
        non_interactive: bool,

        /// List existing namespaces instead of adding
        #[arg(long)]
        list: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Remove a shared namespace
    Remove {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// Namespace ID to remove
        #[arg(long)]
        namespace_id: Option<String>,

        /// Skip interactive prompts (requires --namespace-id)
        #[arg(long)]
        non_interactive: bool,

        /// List namespaces instead of removing
        #[arg(long)]
        list: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum PermissionCommands {
    /// Grant permissions to a user on a namespace
    Grant {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// User ID to grant permissions to
        #[arg(long)]
        user_id: Option<String>,

        /// Namespace ID to grant access to
        #[arg(long)]
        namespace_id: Option<String>,

        /// Permissions to grant (comma-separated: repo:read,repo:write,repo:admin,namespace:read,namespace:write,namespace:admin)
        #[arg(long)]
        permissions: Option<String>,

        /// Skip interactive prompts (requires --user-id, --namespace-id, --permissions)
        #[arg(long)]
        non_interactive: bool,

        /// List existing grants instead of creating
        #[arg(long)]
        list: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Revoke a user's permissions on a namespace
    Revoke {
        /// Data directory for database and repositories
        #[arg(long, default_value = "./data")]
        data_dir: String,

        /// User ID to revoke permissions from
        #[arg(long)]
        user_id: Option<String>,

        /// Namespace ID to revoke access from
        #[arg(long)]
        namespace_id: Option<String>,

        /// Skip interactive prompts (requires --user-id and --namespace-id)
        #[arg(long)]
        non_interactive: bool,

        /// List grants instead of revoking
        #[arg(long)]
        list: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Configure server URL and authentication token
    Login {
        /// Server URL
        #[arg(long)]
        server: Option<String>,

        /// Authentication token
        #[arg(long)]
        token: Option<String>,

        /// Skip interactive prompts
        #[arg(long)]
        non_interactive: bool,
    },
}

#[derive(Subcommand)]
pub enum RepoCommands {
    /// Delete a repository
    Delete {
        /// Repository ID to delete
        #[arg(long)]
        repo_id: Option<String>,

        /// Namespace filter (default: primary)
        #[arg(short, long)]
        namespace: Option<String>,

        /// List repos instead of deleting
        #[arg(long)]
        list: bool,

        /// Skip interactive prompts
        #[arg(long)]
        non_interactive: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Skip confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Clone a repository
    Clone {
        /// Repository to clone (format: namespace/repo or repo ID)
        #[arg(long)]
        repo: Option<String>,

        /// List repos instead of cloning
        #[arg(long)]
        list: bool,

        /// Skip interactive prompts
        #[arg(long)]
        non_interactive: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Set tags on a repository
    Tag {
        /// Repository ID
        #[arg(long)]
        repo_id: Option<String>,

        /// Namespace filter
        #[arg(short, long)]
        namespace: Option<String>,

        /// Tag IDs to set (comma-separated, replaces all existing tags)
        #[arg(long)]
        tags: Option<String>,

        /// List available tags instead of setting
        #[arg(long)]
        list: bool,

        /// Skip interactive prompts
        #[arg(long)]
        non_interactive: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum TagCommands {
    /// Create a new tag
    Create {
        /// Tag name
        #[arg(long)]
        name: Option<String>,

        /// Tag color (hex, e.g., "ff0000")
        #[arg(long)]
        color: Option<String>,

        /// Namespace (default: primary)
        #[arg(short, long)]
        namespace: Option<String>,

        /// List tags instead of creating
        #[arg(long)]
        list: bool,

        /// Skip interactive prompts
        #[arg(long)]
        non_interactive: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Delete a tag
    Delete {
        /// Tag ID to delete
        #[arg(long)]
        tag_id: Option<String>,

        /// Namespace filter
        #[arg(short, long)]
        namespace: Option<String>,

        /// List tags instead of deleting
        #[arg(long)]
        list: bool,

        /// Skip interactive prompts
        #[arg(long)]
        non_interactive: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Skip confirmation
        #[arg(long, short = 'y')]
        yes: bool,

        /// Force delete even if tag has repos
        #[arg(long)]
        force: bool,
    },
}
