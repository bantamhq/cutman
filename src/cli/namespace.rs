use chrono::Utc;
use inquire::Text;
use uuid::Uuid;

use crate::server::validation::validate_namespace_name;
use crate::store::Store;
use crate::types::Namespace;

use super::init_store;
use super::pickers::{confirm_action, pick_namespace};

pub fn run_namespace_add(
    data_dir: String,
    name: Option<String>,
    non_interactive: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let name = if let Some(n) = name {
        validate_namespace_name(&n).map_err(anyhow::Error::msg)?;
        n
    } else if non_interactive {
        anyhow::bail!("--name is required in non-interactive mode");
    } else {
        Text::new("Namespace name:")
            .with_validator(|input: &str| {
                Ok(validate_namespace_name(input)
                    .map(|()| inquire::validator::Validation::Valid)
                    .unwrap_or_else(|e| inquire::validator::Validation::Invalid(e.into())))
            })
            .prompt()?
    };

    if store.get_namespace_by_name(&name)?.is_some() {
        anyhow::bail!("Namespace '{}' already exists", name);
    }

    let namespace = Namespace {
        id: Uuid::new_v4().to_string(),
        name: name.clone(),
        created_at: Utc::now(),
        repo_limit: None,
        storage_limit_bytes: None,
        external_id: None,
    };

    store.create_namespace(&namespace)?;

    println!();
    println!("Created namespace \"{}\"", name);
    println!();

    Ok(())
}

pub fn run_namespace_remove(
    data_dir: String,
    namespace_id: Option<String>,
    non_interactive: bool,
    yes: bool,
) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

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

    if store
        .get_user_by_primary_namespace_id(&namespace.id)?
        .is_some()
    {
        anyhow::bail!(
            "Cannot delete namespace '{}' - it is a user's primary namespace. Delete the user instead.",
            namespace.name
        );
    }

    let confirmed = confirm_action(
        &format!("Delete namespace '{}'?", namespace.name),
        yes,
        non_interactive,
    )?;

    if !confirmed {
        println!("Cancelled.");
        return Ok(());
    }

    store.delete_namespace(&namespace.id)?;

    println!();
    println!("Deleted namespace '{}'", namespace.name);
    println!();

    Ok(())
}
