use chrono::Duration;
use serde::Serialize;

use crate::auth::TokenGenerator;
use crate::store::Store;

use super::init_store;
use super::pickers::{
    TokenDisplay, confirm_action, create_token_for_user, format_relative_time, get_or_pick_user,
    list_tokens, pick_expiration, pick_token, resolve_token_username,
};

#[derive(Serialize)]
struct TokenOutput {
    id: String,
    lookup: String,
    user_id: Option<String>,
    username: Option<String>,
    is_admin: bool,
    created_at: String,
    expires_at: Option<String>,
    last_used_at: Option<String>,
}

impl From<&TokenDisplay> for TokenOutput {
    fn from(display: &TokenDisplay) -> Self {
        Self {
            id: display.token.id.clone(),
            lookup: display.token.token_lookup.clone(),
            user_id: display.token.user_id.clone(),
            username: display.username.clone(),
            is_admin: display.token.is_admin,
            created_at: display.token.created_at.to_rfc3339(),
            expires_at: display.token.expires_at.map(|dt| dt.to_rfc3339()),
            last_used_at: display.token.last_used_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

fn print_tokens_list(tokens: &[TokenDisplay]) {
    if tokens.is_empty() {
        println!("No tokens found.");
        return;
    }
    println!();
    for token in tokens {
        let user = token.username.as_deref().unwrap_or("admin");
        let created = format_relative_time(&token.token.created_at);
        let last_used = match &token.token.last_used_at {
            Some(dt) => format_relative_time(dt),
            None => "never used".to_string(),
        };
        println!(
            "  cutman_{}...  {}  created {}  {}",
            &token.token.token_lookup, user, created, last_used
        );
    }
    println!();
}

pub fn run_token_create(
    data_dir: String,
    user_id: Option<String>,
    expires_days: Option<i64>,
    non_interactive: bool,
    list: bool,
    json: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    if list {
        let tokens = list_tokens(&store)?;
        if json {
            let output: Vec<TokenOutput> = tokens.iter().map(TokenOutput::from).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            print_tokens_list(&tokens);
        }
        return Ok(());
    }

    let (user_id, username) = match get_or_pick_user(&store, user_id, non_interactive)? {
        Some((user, name)) => (Some(user.id), name),
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
    let (token, raw_token) = create_token_for_user(&generator, user_id, expires_in)?;
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
    list: bool,
    json: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    if list {
        let tokens = list_tokens(&store)?;
        if json {
            let output: Vec<TokenOutput> = tokens.iter().map(TokenOutput::from).collect();
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            print_tokens_list(&tokens);
        }
        return Ok(());
    }

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
