use serde::Serialize;

use crate::store::Store;

use super::init_store;

#[derive(Serialize)]
struct ServerInfo {
    users: i32,
    namespaces: i32,
    namespaces_primary: i32,
    namespaces_shared: i32,
    tokens: i32,
    repos: i32,
}

pub fn run_info(data_dir: String, json: bool) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let users = store.list_users("", 10000)?;
    let namespaces = store.list_namespaces("", 10000)?;
    let tokens = store.list_tokens("", 10000)?;

    let mut primary_count = 0;
    let mut repo_count = 0;

    for ns in &namespaces {
        if store.get_user_by_primary_namespace_id(&ns.id)?.is_some() {
            primary_count += 1;
        }
        repo_count += store.list_repos(&ns.id, "", 10000)?.len() as i32;
    }

    let shared_count = namespaces.len() as i32 - primary_count;

    let info = ServerInfo {
        users: users.len() as i32,
        namespaces: namespaces.len() as i32,
        namespaces_primary: primary_count,
        namespaces_shared: shared_count,
        tokens: tokens.len() as i32,
        repos: repo_count,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!();
        println!("Cutman Server Status");
        println!("{}", "─".repeat(20));
        println!("Users:       {}", info.users);
        println!(
            "Namespaces:  {} ({} primary, {} shared)",
            info.namespaces, info.namespaces_primary, info.namespaces_shared
        );
        println!("Tokens:      {}", info.tokens);
        println!("Repos:       {}", info.repos);
        println!();
    }

    Ok(())
}
