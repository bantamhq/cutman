use chrono::Utc;
use inquire::{Confirm, Text};
use uuid::Uuid;

use crate::auth::TokenGenerator;
use crate::server::validation::validate_namespace_name;
use crate::store::Store;
use crate::types::{Namespace, User};

use super::init_store;
use super::pickers::{confirm_action, create_token_for_user, get_or_pick_user, pick_expiration};

pub fn run_user_add(
    data_dir: String,
    username: Option<String>,
    create_token_flag: bool,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let username = if let Some(name) = username {
        validate_namespace_name(&name).map_err(anyhow::Error::msg)?;
        name
    } else if non_interactive {
        anyhow::bail!("--username is required in non-interactive mode");
    } else {
        Text::new("Username:")
            .with_validator(|input: &str| {
                Ok(validate_namespace_name(input)
                    .map(|()| inquire::validator::Validation::Valid)
                    .unwrap_or_else(|e| inquire::validator::Validation::Invalid(e.into())))
            })
            .prompt()?
    };

    if store.get_namespace_by_name(&username)?.is_some() {
        anyhow::bail!("Namespace '{}' already exists", username);
    }

    let now = Utc::now();
    let namespace_id = Uuid::new_v4().to_string();
    let user_id = Uuid::new_v4().to_string();

    let namespace = Namespace {
        id: namespace_id.clone(),
        name: username.clone(),
        created_at: now,
        repo_limit: None,
        storage_limit_bytes: None,
        external_id: None,
    };

    let user = User {
        id: user_id.clone(),
        primary_namespace_id: namespace_id,
        created_at: now,
        updated_at: now,
    };

    store.create_namespace(&namespace)?;
    store.create_user(&user)?;

    println!();
    println!(
        "Created user \"{}\" with namespace \"{}\"",
        username, username
    );

    let should_create_token = if create_token_flag {
        true
    } else if non_interactive {
        false
    } else {
        Confirm::new("Create access token?")
            .with_default(true)
            .prompt()?
    };

    if should_create_token {
        let expires_in = if non_interactive {
            None
        } else {
            match pick_expiration()? {
                Some(exp) => exp,
                None => {
                    println!("Token creation cancelled.");
                    return Ok(());
                }
            }
        };

        let generator = TokenGenerator::new();
        let (token, raw_token) = create_token_for_user(&generator, Some(user_id), expires_in)?;
        store.create_token(&token)?;

        println!();
        println!("Token created: {raw_token}");
        println!("  Save this now - it cannot be retrieved later.");
    }

    println!();

    Ok(())
}

pub fn run_user_remove(
    data_dir: String,
    user_id: Option<String>,
    non_interactive: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let (user, username) = match get_or_pick_user(&store, user_id, non_interactive)? {
        Some(result) => result,
        None => return Ok(()),
    };

    let confirmed = confirm_action(
        &format!(
            "Delete user '{}'? This will also delete their namespace, tokens, and grants.",
            username
        ),
        yes,
        non_interactive,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    for token in store.list_user_tokens(&user.id)? {
        store.delete_token(&token.id)?;
    }

    for grant in store.list_user_namespace_grants(&user.id)? {
        store.delete_namespace_grant(&user.id, &grant.namespace_id)?;
    }

    for grant in store.list_user_repo_grants(&user.id)? {
        store.delete_repo_grant(&user.id, &grant.repo_id)?;
    }

    store.delete_user(&user.id)?;
    store.delete_namespace(&user.primary_namespace_id)?;

    println!();
    println!("Deleted user '{}'", username);
    println!();

    Ok(())
}
