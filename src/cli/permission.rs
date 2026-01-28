use chrono::Utc;
use serde::Serialize;

use crate::store::Store;
use crate::types::{NamespaceGrant, Permission};

use super::init_store;
use super::pickers::{
    GrantDisplay, UserDisplay, confirm_action, get_or_pick_user, list_all_grants, pick_grant,
    pick_namespace, pick_permissions,
};

#[derive(Serialize)]
struct GrantOutput {
    user_id: String,
    username: String,
    namespace_id: String,
    namespace_name: String,
    permissions: Vec<&'static str>,
}

fn grant_to_output(user: &UserDisplay, grant: &GrantDisplay) -> GrantOutput {
    GrantOutput {
        user_id: user.user.id.clone(),
        username: user.namespace_name.clone(),
        namespace_id: grant.grant.namespace_id.clone(),
        namespace_name: grant.namespace_name.clone(),
        permissions: grant.grant.allow_bits.to_strings(),
    }
}

fn print_grants_list(all_grants: &[(UserDisplay, Vec<GrantDisplay>)]) {
    if all_grants.is_empty() {
        println!("No grants found.");
        return;
    }
    println!();
    for (user, grants) in all_grants {
        println!("  {} ({}):", user.namespace_name, &user.user.id[..8]);
        for grant in grants {
            println!(
                "    {} [{}]",
                grant.namespace_name,
                grant.grant.allow_bits.to_strings().join(", ")
            );
        }
    }
    println!();
}

pub fn run_permission_grant(
    data_dir: String,
    user_id: Option<String>,
    namespace_id: Option<String>,
    permissions: Option<String>,
    non_interactive: bool,
    list: bool,
    json: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    if list {
        let all_grants = list_all_grants(&store)?;
        if json {
            let output: Vec<GrantOutput> = all_grants
                .iter()
                .flat_map(|(user, grants)| grants.iter().map(|g| grant_to_output(user, g)))
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            print_grants_list(&all_grants);
        }
        return Ok(());
    }

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
    list: bool,
    json: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    if list {
        let all_grants = list_all_grants(&store)?;
        if json {
            let output: Vec<GrantOutput> = all_grants
                .iter()
                .flat_map(|(user, grants)| grants.iter().map(|g| grant_to_output(user, g)))
                .collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            print_grants_list(&all_grants);
        }
        return Ok(());
    }

    let (user, username) = match get_or_pick_user(&store, user_id, non_interactive)? {
        Some(result) => result,
        None => return Ok(()),
    };

    let (grant, namespace_name) = if let Some(ns_id) = namespace_id {
        let grant = store
            .get_namespace_grant(&user.id, &ns_id)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Grant not found for user {} on namespace {}",
                    user.id,
                    ns_id
                )
            })?;
        let ns = store.get_namespace(&ns_id)?;
        let name = ns
            .map(|n| n.name)
            .unwrap_or_else(|| "<unknown>".to_string());
        (grant, name)
    } else if non_interactive {
        anyhow::bail!("--namespace-id is required in non-interactive mode");
    } else {
        match pick_grant(&store, &user.id)? {
            Some(grant) => {
                let ns = store.get_namespace(&grant.namespace_id)?;
                let name = ns
                    .map(|n| n.name)
                    .unwrap_or_else(|| "<unknown>".to_string());
                (grant, name)
            }
            None => return Ok(()),
        }
    };

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
