use std::fmt;

use chrono::{DateTime, Duration, Utc};
use inquire::{InquireError, MultiSelect, Select};
use uuid::Uuid;

use crate::auth::TokenGenerator;
use crate::store::Store;
use crate::types::{Namespace, NamespaceGrant, Permission, Repo, RepoGrant, Tag, Token, User};

/// User with resolved namespace name for display
pub struct UserDisplay {
    pub user: User,
    pub namespace_name: String,
}

impl fmt::Display for UserDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}...)", self.namespace_name, &self.user.id[..8])
    }
}

/// Namespace with ownership info for display
pub struct NamespaceDisplay {
    pub namespace: Namespace,
    pub has_owner: bool,
}

impl fmt::Display for NamespaceDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = if self.has_owner {
            "[owned]"
        } else {
            "[shared]"
        };
        write!(f, "{} {}", self.namespace.name, label)
    }
}

/// Token with resolved username for display
pub struct TokenDisplay {
    pub token: Token,
    pub username: Option<String>,
}

impl fmt::Display for TokenDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let user = self.username.as_deref().unwrap_or("admin");
        let created = format_relative_time(&self.token.created_at);
        let last_used = match &self.token.last_used_at {
            Some(dt) => format_relative_time(dt),
            None => "never used".to_string(),
        };
        write!(
            f,
            "cutman_{}...  {}  created {}  {}",
            &self.token.token_lookup, user, created, last_used
        )
    }
}

/// Namespace grant with resolved namespace name for display
pub struct GrantDisplay {
    pub grant: NamespaceGrant,
    pub namespace_name: String,
}

impl fmt::Display for GrantDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}]",
            self.namespace_name,
            self.grant.allow_bits.to_strings().join(", ")
        )
    }
}

/// Token expiration option for display
#[derive(Clone)]
pub struct ExpirationOption {
    pub label: &'static str,
    pub days: Option<i64>,
}

impl fmt::Display for ExpirationOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

/// Repo with resolved namespace name for display
pub struct RepoDisplay {
    pub repo: Repo,
    pub namespace_name: String,
}

impl fmt::Display for RepoDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace_name, self.repo.name)
    }
}

/// Tag for display (shows name with optional color)
pub struct TagDisplay {
    pub tag: Tag,
}

impl fmt::Display for TagDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.tag.color {
            Some(c) => write!(f, "{} [{}]", self.tag.name, c),
            None => write!(f, "{}", self.tag.name),
        }
    }
}

/// Repo grant with resolved repo name for display
pub struct RepoGrantDisplay {
    pub grant: RepoGrant,
    pub repo_name: String,
    pub namespace_name: String,
}

impl fmt::Display for RepoGrantDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{} [{}]",
            self.namespace_name,
            self.repo_name,
            self.grant.allow_bits.to_strings().join(", ")
        )
    }
}

/// Print a list of tags with optional color formatting
pub fn print_tags_list(tags: &[Tag]) {
    if tags.is_empty() {
        println!("No tags found.");
        return;
    }
    println!();
    for tag in tags {
        let display = TagDisplay { tag: tag.clone() };
        println!("  {display}");
    }
    println!();
}

/// Convert repos to display items using a namespace map
pub fn repos_to_displays(
    repos: Vec<Repo>,
    namespace_map: &std::collections::HashMap<String, String>,
) -> Vec<RepoDisplay> {
    repos
        .into_iter()
        .map(|repo| {
            let namespace_name = namespace_map
                .get(&repo.namespace_id)
                .cloned()
                .unwrap_or_default();
            RepoDisplay {
                repo,
                namespace_name,
            }
        })
        .collect()
}

/// Format a datetime as relative time (e.g., "2 days ago")
#[must_use]
pub fn format_relative_time(dt: &DateTime<Utc>) -> String {
    let now = Utc::now();
    let diff = now.signed_duration_since(*dt);

    if diff.num_seconds() < 0 {
        return "in the future".to_string();
    }

    if diff.num_seconds() < 60 {
        return "just now".to_string();
    }

    if diff.num_minutes() < 60 {
        let mins = diff.num_minutes();
        return if mins == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{mins} minutes ago")
        };
    }

    if diff.num_hours() < 24 {
        let hours = diff.num_hours();
        return if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{hours} hours ago")
        };
    }

    if diff.num_days() < 30 {
        let days = diff.num_days();
        return if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        };
    }

    if diff.num_days() < 365 {
        let months = diff.num_days() / 30;
        return if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{months} months ago")
        };
    }

    let years = diff.num_days() / 365;
    if years == 1 {
        "1 year ago".to_string()
    } else {
        format!("{years} years ago")
    }
}

/// Resolve a namespace ID to its name, returning "<unknown>" if not found
#[must_use]
pub fn resolve_namespace_name(store: &impl Store, namespace_id: &str) -> String {
    store
        .get_namespace(namespace_id)
        .ok()
        .flatten()
        .map(|ns| ns.name)
        .unwrap_or_else(|| "<unknown>".to_string())
}

/// Build a repo display name in "namespace/repo" format
pub fn resolve_repo_display_name(store: &impl Store, repo_id: &str) -> anyhow::Result<String> {
    match store.get_repo_by_id(repo_id)? {
        Some(repo) => {
            let ns_name = resolve_namespace_name(store, &repo.namespace_id);
            Ok(format!("{}/{}", ns_name, repo.name))
        }
        None => Ok("<unknown>".to_string()),
    }
}

/// Load users with their namespace names
fn load_users_with_namespaces(store: &impl Store) -> anyhow::Result<Vec<UserDisplay>> {
    let users = store.list_users("", 1000)?;
    let displays = users
        .into_iter()
        .map(|user| {
            let namespace_name = resolve_namespace_name(store, &user.primary_namespace_id);
            UserDisplay {
                user,
                namespace_name,
            }
        })
        .collect();
    Ok(displays)
}

/// Load namespaces with ownership info
fn load_namespaces_with_ownership(
    store: &impl Store,
    exclude_owned: bool,
) -> anyhow::Result<Vec<NamespaceDisplay>> {
    let namespaces = store.list_namespaces("", 1000)?;
    let mut displays = Vec::new();

    for namespace in namespaces {
        let has_owner = store
            .get_user_by_primary_namespace_id(&namespace.id)?
            .is_some();

        if exclude_owned && has_owner {
            continue;
        }

        displays.push(NamespaceDisplay {
            namespace,
            has_owner,
        });
    }

    Ok(displays)
}

/// Load tokens with resolved usernames
fn load_tokens_with_users(store: &impl Store) -> anyhow::Result<Vec<TokenDisplay>> {
    let tokens = store.list_tokens("", 1000)?;
    let mut displays = Vec::with_capacity(tokens.len());

    for token in tokens {
        let username = match &token.user_id {
            Some(user_id) => store
                .get_user(user_id)?
                .map(|u| resolve_namespace_name(store, &u.primary_namespace_id)),
            None => None,
        };

        displays.push(TokenDisplay { token, username });
    }

    Ok(displays)
}

/// Load grants for a user with namespace names
fn load_user_grants_with_names(
    store: &impl Store,
    user_id: &str,
) -> anyhow::Result<Vec<GrantDisplay>> {
    let grants = store.list_user_namespace_grants(user_id)?;
    let displays = grants
        .into_iter()
        .map(|grant| {
            let namespace_name = resolve_namespace_name(store, &grant.namespace_id);
            GrantDisplay {
                grant,
                namespace_name,
            }
        })
        .collect();
    Ok(displays)
}

/// Pick a user from the list
pub fn pick_user(store: &impl Store) -> anyhow::Result<Option<User>> {
    let users = load_users_with_namespaces(store)?;

    if users.is_empty() {
        println!("No users found.");
        return Ok(None);
    }

    let selection = Select::new("Select user:", users)
        .with_page_size(15)
        .with_help_message("Type to filter, Enter to select")
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(display) => Ok(Some(display.user)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Pick a namespace from the list
pub fn pick_namespace(
    store: &impl Store,
    exclude_owned: bool,
) -> anyhow::Result<Option<Namespace>> {
    let namespaces = load_namespaces_with_ownership(store, exclude_owned)?;

    if namespaces.is_empty() {
        if exclude_owned {
            println!("No shared namespaces found.");
        } else {
            println!("No namespaces found.");
        }
        return Ok(None);
    }

    let selection = Select::new("Select namespace:", namespaces)
        .with_page_size(15)
        .with_help_message("Type to filter, Enter to select")
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(display) => Ok(Some(display.namespace)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Pick a token from the list
pub fn pick_token(store: &impl Store) -> anyhow::Result<Option<Token>> {
    let tokens = load_tokens_with_users(store)?;

    if tokens.is_empty() {
        println!("No tokens found.");
        return Ok(None);
    }

    let selection = Select::new("Select token:", tokens)
        .with_page_size(15)
        .with_help_message("Type to filter, Enter to select")
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(display) => Ok(Some(display.token)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Pick a grant for a specific user
pub fn pick_grant(store: &impl Store, user_id: &str) -> anyhow::Result<Option<NamespaceGrant>> {
    let grants = load_user_grants_with_names(store, user_id)?;

    if grants.is_empty() {
        println!("No grants found for this user.");
        return Ok(None);
    }

    let selection = Select::new("Select grant to revoke:", grants)
        .with_page_size(15)
        .with_help_message("Type to filter, Enter to select")
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(display) => Ok(Some(display.grant)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Pick permissions using multi-select
pub fn pick_permissions() -> anyhow::Result<Option<Permission>> {
    let options = vec![
        "repo:read",
        "repo:write",
        "repo:admin",
        "namespace:read",
        "namespace:write",
        "namespace:admin",
    ];

    let selection = MultiSelect::new("Permissions to grant:", options)
        .with_page_size(6)
        .with_help_message("Space to toggle, Enter to confirm")
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(selected) => {
            if selected.is_empty() {
                return Ok(None);
            }
            let refs: Vec<&str> = selected.into_iter().collect();
            Ok(Permission::parse_many(&refs))
        }
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Pick token expiration
pub fn pick_expiration() -> anyhow::Result<Option<Option<Duration>>> {
    let options = vec![
        ExpirationOption {
            label: "30 days",
            days: Some(30),
        },
        ExpirationOption {
            label: "90 days",
            days: Some(90),
        },
        ExpirationOption {
            label: "1 year",
            days: Some(365),
        },
        ExpirationOption {
            label: "Never",
            days: None,
        },
    ];

    let selection = Select::new("Token expiration:", options)
        .with_page_size(4)
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(opt) => Ok(Some(opt.days.map(Duration::days))),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Get all users for listing
pub fn list_users(store: &impl Store) -> anyhow::Result<Vec<UserDisplay>> {
    load_users_with_namespaces(store)
}

/// Get all namespaces for listing
pub fn list_namespaces(store: &impl Store) -> anyhow::Result<Vec<NamespaceDisplay>> {
    load_namespaces_with_ownership(store, false)
}

/// Get all tokens for listing
pub fn list_tokens(store: &impl Store) -> anyhow::Result<Vec<TokenDisplay>> {
    load_tokens_with_users(store)
}

/// Get all grants for listing
pub fn list_all_grants(
    store: &impl Store,
) -> anyhow::Result<Vec<(UserDisplay, Vec<GrantDisplay>)>> {
    let users = load_users_with_namespaces(store)?;
    let mut result = Vec::new();

    for user_display in users {
        let grants = load_user_grants_with_names(store, &user_display.user.id)?;
        if !grants.is_empty() {
            result.push((user_display, grants));
        }
    }

    Ok(result)
}

fn resolve_user_with_name(store: &impl Store, user_id: &str) -> anyhow::Result<(User, String)> {
    let user = store
        .get_user(user_id)?
        .ok_or_else(|| anyhow::anyhow!("User not found: {}", user_id))?;
    let name = resolve_namespace_name(store, &user.primary_namespace_id);
    Ok((user, name))
}

/// Get a user by ID or interactively pick one
pub fn get_or_pick_user(
    store: &impl Store,
    user_id: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<Option<(User, String)>> {
    if let Some(id) = user_id {
        Ok(Some(resolve_user_with_name(store, &id)?))
    } else if non_interactive {
        anyhow::bail!("--user-id is required in non-interactive mode");
    } else {
        match pick_user(store)? {
            Some(user) => {
                let name = resolve_namespace_name(store, &user.primary_namespace_id);
                Ok(Some((user, name)))
            }
            None => Ok(None),
        }
    }
}

/// Resolve a token's username from its user_id
pub fn resolve_token_username(store: &impl Store, token: &Token) -> anyhow::Result<Option<String>> {
    if let Some(ref uid) = token.user_id {
        if let Some(user) = store.get_user(uid)? {
            return Ok(store
                .get_namespace(&user.primary_namespace_id)?
                .map(|n| n.name));
        }
    }
    Ok(None)
}

/// Request confirmation for a destructive operation
pub fn confirm_action(message: &str, yes: bool, non_interactive: bool) -> anyhow::Result<bool> {
    if yes {
        Ok(true)
    } else if non_interactive {
        anyhow::bail!("--yes is required for destructive operations in non-interactive mode");
    } else {
        Ok(inquire::Confirm::new(message)
            .with_default(false)
            .prompt()?)
    }
}

/// Create a new token record for a user
pub fn create_token_for_user(
    generator: &TokenGenerator,
    user_id: Option<String>,
    expires_in: Option<Duration>,
) -> anyhow::Result<(Token, String)> {
    let (raw_token, lookup, hash) = generator.generate()?;
    let now = Utc::now();
    let token = Token {
        id: Uuid::new_v4().to_string(),
        token_hash: hash,
        token_lookup: lookup,
        is_admin: user_id.is_none(),
        user_id,
        created_at: now,
        expires_at: expires_in.map(|d| now + d),
        last_used_at: None,
    };
    Ok((token, raw_token))
}

/// Load all repos with their namespace names
fn load_repos_with_namespaces(store: &impl Store) -> anyhow::Result<Vec<RepoDisplay>> {
    let namespaces = store.list_namespaces("", 1000)?;
    let namespace_map: std::collections::HashMap<String, String> =
        namespaces.into_iter().map(|ns| (ns.id, ns.name)).collect();

    let mut all_repos = Vec::new();
    for ns_id in namespace_map.keys() {
        let repos = store.list_repos(ns_id, "", 1000)?;
        all_repos.extend(repos);
    }

    Ok(repos_to_displays(all_repos, &namespace_map))
}

/// Load repo grants for a user with repo names
fn load_user_repo_grants_with_names(
    store: &impl Store,
    user_id: &str,
) -> anyhow::Result<Vec<RepoGrantDisplay>> {
    let grants = store.list_user_repo_grants(user_id)?;
    let mut displays = Vec::with_capacity(grants.len());

    for grant in grants {
        let (repo_name, namespace_name) = match store.get_repo_by_id(&grant.repo_id)? {
            Some(repo) => (repo.name.clone(), resolve_namespace_name(store, &repo.namespace_id)),
            None => ("<deleted>".to_string(), "<unknown>".to_string()),
        };

        displays.push(RepoGrantDisplay {
            grant,
            repo_name,
            namespace_name,
        });
    }

    Ok(displays)
}

/// Pick a repo from all available repos
pub fn pick_repo(store: &impl Store) -> anyhow::Result<Option<Repo>> {
    let repos = load_repos_with_namespaces(store)?;

    if repos.is_empty() {
        println!("No repositories found.");
        return Ok(None);
    }

    let selection = Select::new("Select repository:", repos)
        .with_page_size(15)
        .with_help_message("Type to filter, Enter to select")
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(display) => Ok(Some(display.repo)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Pick a repo grant for a specific user
pub fn pick_repo_grant(store: &impl Store, user_id: &str) -> anyhow::Result<Option<RepoGrant>> {
    let grants = load_user_repo_grants_with_names(store, user_id)?;

    if grants.is_empty() {
        println!("No repo grants found for this user.");
        return Ok(None);
    }

    let selection = Select::new("Select repo grant to revoke:", grants)
        .with_page_size(15)
        .with_help_message("Type to filter, Enter to select")
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(display) => Ok(Some(display.grant)),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Pick repo permissions using multi-select
pub fn pick_repo_permissions() -> anyhow::Result<Option<Permission>> {
    let options = vec!["repo:read", "repo:write", "repo:admin"];

    let selection = MultiSelect::new("Permissions to grant:", options)
        .with_page_size(3)
        .with_help_message("Space to toggle, Enter to confirm")
        .with_vim_mode(true)
        .prompt();

    match selection {
        Ok(selected) => {
            if selected.is_empty() {
                return Ok(None);
            }
            let refs: Vec<&str> = selected.into_iter().collect();
            Ok(Permission::parse_many(&refs))
        }
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
