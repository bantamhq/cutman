use crate::server::response::{ApiError, StoreResultExt};
use crate::store::Store;
use crate::types::{Permission, Repo, User};

/// Returns true if user has the required permission for a namespace.
/// Primary namespace owners have full access.
pub fn check_namespace_permission(
    store: &dyn Store,
    user: &User,
    namespace_id: &str,
    required: Permission,
) -> Result<bool, ApiError> {
    if user.primary_namespace_id == namespace_id {
        return Ok(true);
    }

    let grant = store
        .get_namespace_grant(&user.id, namespace_id)
        .api_err("Failed to check namespace grant")?;

    Ok(grant
        .map(|g| {
            g.allow_bits
                .expand_implied()
                .difference(g.deny_bits)
                .has(required)
        })
        .unwrap_or(false))
}

/// Returns true if user has the required permission for a repo.
/// Checks both namespace-level and repo-level grants.
pub fn check_repo_permission(
    store: &dyn Store,
    user: &User,
    repo: &Repo,
    required: Permission,
) -> Result<bool, ApiError> {
    if user.primary_namespace_id == repo.namespace_id {
        return Ok(true);
    }

    let ns_grant = store
        .get_namespace_grant(&user.id, &repo.namespace_id)
        .api_err("Failed to check namespace grant")?;

    let repo_grant = store
        .get_repo_grant(&user.id, &repo.id)
        .api_err("Failed to check repo grant")?;

    let mut allow = Permission::default();
    let mut deny = Permission::default();

    if let Some(grant) = ns_grant {
        allow = allow.union(grant.allow_bits.expand_implied());
        deny = deny.union(grant.deny_bits);
    }

    if let Some(grant) = repo_grant {
        allow = allow.union(grant.allow_bits.expand_implied());
        deny = deny.union(grant.deny_bits);
    }

    Ok(allow.difference(deny).has(required))
}

/// Resolves namespace from optional name or uses user's primary namespace.
pub fn resolve_namespace_id(
    store: &dyn Store,
    user: &User,
    namespace_name: Option<&str>,
) -> Result<String, ApiError> {
    match namespace_name {
        Some(name) => {
            let ns = store
                .get_namespace_by_name(name)
                .api_err("Failed to lookup namespace")?
                .ok_or_else(|| ApiError::not_found("Namespace not found"))?;
            Ok(ns.id)
        }
        None => Ok(user.primary_namespace_id.clone()),
    }
}

/// Check if user has the required namespace permission, returning forbidden error if not.
pub fn require_namespace_permission(
    store: &dyn Store,
    user: &User,
    namespace_id: &str,
    required: Permission,
) -> Result<(), ApiError> {
    if !check_namespace_permission(store, user, namespace_id, required)? {
        return Err(ApiError::forbidden("Insufficient namespace permissions"));
    }
    Ok(())
}

/// Check if user has the required repo permission, returning forbidden error if not.
pub fn require_repo_permission(
    store: &dyn Store,
    user: &User,
    repo: &Repo,
    required: Permission,
) -> Result<(), ApiError> {
    if !check_repo_permission(store, user, repo, required)? {
        return Err(ApiError::forbidden("Insufficient repository permissions"));
    }
    Ok(())
}
