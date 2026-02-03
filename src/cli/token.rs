use chrono::Duration;

use crate::auth::TokenGenerator;
use crate::store::Store;

use super::init_store;
use super::pickers::{
    confirm_action, create_token_for_principal, get_or_pick_principal, pick_expiration, pick_token,
    resolve_token_username,
};

pub fn run_token_create(
    data_dir: String,
    principal_id: Option<String>,
    expires_days: Option<i64>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let (principal_id, username) = match get_or_pick_principal(&store, principal_id, non_interactive)? {
        Some((principal, name)) => (Some(principal.id), name),
        None => return Ok(()),
    };

    let expires_in = if let Some(days) = expires_days {
        if days <= 0 {
            None
        } else {
            Some(Duration::days(days))
        }
    } else if non_interactive {
        None
    } else {
        match pick_expiration()? {
            Some(exp) => exp,
            None => {
                println!("Cancelled.");
                return Ok(());
            }
        }
    };

    let generator = TokenGenerator::new();
    let (token, raw_token) = create_token_for_principal(&generator, principal_id, expires_in)?;
    store.create_token(&token)?;

    println!();
    println!("Token created for '{}': {}", username, raw_token);
    println!("  Save this now - it cannot be retrieved later.");
    println!();

    Ok(())
}

pub fn run_token_revoke(
    data_dir: String,
    token_id: Option<String>,
    non_interactive: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let (token, username) = if let Some(id) = token_id {
        let token = store
            .get_token_by_id(&id)?
            .ok_or_else(|| anyhow::anyhow!("Token not found: {}", id))?;
        let username = resolve_token_username(&store, &token)?;
        (token, username)
    } else if non_interactive {
        anyhow::bail!("--token-id is required in non-interactive mode");
    } else {
        match pick_token(&store)? {
            Some(token) => {
                let username = resolve_token_username(&store, &token)?;
                (token, username)
            }
            None => return Ok(()),
        }
    };

    let user_label = username.as_deref().unwrap_or("admin");

    let confirmed = confirm_action(
        &format!(
            "Revoke token cutman_{}... for user '{}'?",
            &token.token_lookup, user_label
        ),
        yes,
        non_interactive,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    store.delete_token(&token.id)?;

    println!();
    println!("Token revoked.");
    println!();

    Ok(())
}
