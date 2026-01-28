use chrono::Utc;

use crate::store::Store;
use crate::types::{NamespaceGrant, Permission, RepoGrant};

use super::init_store;
use super::pickers::{
    confirm_action, get_or_pick_user, pick_grant, pick_namespace, pick_permissions, pick_repo,
    pick_repo_grant, pick_repo_permissions, resolve_namespace_name, resolve_repo_display_name,
};

pub fn run_permission_grant(
    data_dir: String,
    user_id: Option<String>,
    namespace_id: Option<String>,
    permissions: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let (user, username) = match get_or_pick_user(&store, user_id, non_interactive)? {
        Some(result) => result,
        None => return Ok(()),
    };

    let namespace = if let Some(id) = namespace_id {
        store
            .get_namespace(&id)?
            .ok_or_else(|| anyhow::anyhow!("Namespace not found: {}", id))?
    } else if non_interactive {
        anyhow::bail!("--namespace-id is required in non-interactive mode");
    } else {
        match pick_namespace(&store, true)? {
            Some(ns) => ns,
            None => return Ok(()),
        }
    };

    let allow_bits = if let Some(perms_str) = permissions {
        let perms: Vec<&str> = perms_str.split(',').map(str::trim).collect();
        Permission::parse_many(&perms)
            .ok_or_else(|| anyhow::anyhow!("Invalid permission string: {}", perms_str))?
    } else if non_interactive {
        anyhow::bail!("--permissions is required in non-interactive mode");
    } else {
        match pick_permissions()? {
            Some(perms) => perms,
            None => {
                println!("No permissions selected.");
                return Ok(());
            }
        }
    };

    let now = Utc::now();
    let grant = NamespaceGrant {
        user_id: user.id.clone(),
        namespace_id: namespace.id.clone(),
        allow_bits,
        deny_bits: Permission::default(),
        created_at: now,
        updated_at: now,
    };

    store.upsert_namespace_grant(&grant)?;

    println!();
    println!(
        "Granted {} access to namespace \"{}\" with: {}",
        username,
        namespace.name,
        allow_bits.to_strings().join(", ")
    );
    println!();

    Ok(())
}

pub fn run_permission_revoke(
    data_dir: String,
    user_id: Option<String>,
    namespace_id: Option<String>,
    non_interactive: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let (user, username) = match get_or_pick_user(&store, user_id, non_interactive)? {
        Some(result) => result,
        None => return Ok(()),
    };

    let grant = if let Some(ns_id) = namespace_id {
        store
            .get_namespace_grant(&user.id, &ns_id)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Grant not found for user {} on namespace {}",
                    user.id,
                    ns_id
                )
            })?
    } else if non_interactive {
        anyhow::bail!("--namespace-id is required in non-interactive mode");
    } else {
        match pick_grant(&store, &user.id)? {
            Some(grant) => grant,
            None => return Ok(()),
        }
    };

    let namespace_name = resolve_namespace_name(&store, &grant.namespace_id);

    let confirmed = confirm_action(
        &format!("Revoke {}'s access to '{}'?", username, namespace_name),
        yes,
        non_interactive,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    store.delete_namespace_grant(&grant.user_id, &grant.namespace_id)?;

    println!();
    println!("Revoked grant.");
    println!();

    Ok(())
}

pub fn run_permission_repo_grant(
    data_dir: String,
    user_id: Option<String>,
    repo_id: Option<String>,
    permissions: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let (user, username) = match get_or_pick_user(&store, user_id, non_interactive)? {
        Some(result) => result,
        None => return Ok(()),
    };

    let repo = if let Some(id) = repo_id {
        store
            .get_repo_by_id(&id)?
            .ok_or_else(|| anyhow::anyhow!("Repository not found: {}", id))?
    } else if non_interactive {
        anyhow::bail!("--repo-id is required in non-interactive mode");
    } else {
        match pick_repo(&store)? {
            Some(r) => r,
            None => return Ok(()),
        }
    };

    let repo_namespace = resolve_namespace_name(&store, &repo.namespace_id);

    let allow_bits = if let Some(perms_str) = permissions {
        let perms: Vec<&str> = perms_str.split(',').map(str::trim).collect();
        Permission::parse_many(&perms)
            .ok_or_else(|| anyhow::anyhow!("Invalid permission string: {}", perms_str))?
    } else if non_interactive {
        anyhow::bail!("--permissions is required in non-interactive mode");
    } else {
        match pick_repo_permissions()? {
            Some(perms) => perms,
            None => {
                println!("No permissions selected.");
                return Ok(());
            }
        }
    };

    let now = Utc::now();
    let grant = RepoGrant {
        user_id: user.id.clone(),
        repo_id: repo.id.clone(),
        allow_bits,
        deny_bits: Permission::default(),
        created_at: now,
        updated_at: now,
    };

    store.upsert_repo_grant(&grant)?;

    println!();
    println!(
        "Granted {} access to repo \"{}/{}\" with: {}",
        username,
        repo_namespace,
        repo.name,
        allow_bits.to_strings().join(", ")
    );
    println!();

    Ok(())
}

pub fn run_permission_repo_revoke(
    data_dir: String,
    user_id: Option<String>,
    repo_id: Option<String>,
    non_interactive: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let (user, username) = match get_or_pick_user(&store, user_id, non_interactive)? {
        Some(result) => result,
        None => return Ok(()),
    };

    let grant = if let Some(r_id) = repo_id {
        store.get_repo_grant(&user.id, &r_id)?.ok_or_else(|| {
            anyhow::anyhow!("Grant not found for user {} on repo {}", user.id, r_id)
        })?
    } else if non_interactive {
        anyhow::bail!("--repo-id is required in non-interactive mode");
    } else {
        match pick_repo_grant(&store, &user.id)? {
            Some(grant) => grant,
            None => return Ok(()),
        }
    };

    let repo_display_name = resolve_repo_display_name(&store, &grant.repo_id)?;

    let confirmed = confirm_action(
        &format!("Revoke {}'s access to '{}'?", username, repo_display_name),
        yes,
        non_interactive,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    store.delete_repo_grant(&grant.user_id, &grant.repo_id)?;

    println!();
    println!("Revoked repo grant.");
    println!();

    Ok(())
}
