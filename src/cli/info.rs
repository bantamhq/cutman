use serde::Serialize;

use crate::store::Store;

use super::init_store;

#[derive(Serialize)]
struct ServerInfo {
    principals: i32,
    namespaces: i32,
    namespaces_primary: i32,
    namespaces_shared: i32,
    tokens: i32,
    repos: i32,
}

#[derive(Serialize)]
struct PrincipalOutput {
    id: String,
    username: String,
    created_at: String,
}

#[derive(Serialize)]
struct NamespaceOutput {
    id: String,
    name: String,
    is_shared: bool,
    created_at: String,
}

#[derive(Serialize)]
struct TokenOutput {
    id: String,
    lookup: String,
    principal_id: Option<String>,
    username: Option<String>,
    is_admin: bool,
    created_at: String,
    expires_at: Option<String>,
    last_used_at: Option<String>,
}

#[derive(Serialize)]
struct GrantOutput {
    principal_id: String,
    username: String,
    namespace_id: String,
    namespace_name: String,
    permissions: Vec<&'static str>,
}

#[derive(Serialize)]
struct DetailedServerInfo {
    principals: Vec<PrincipalOutput>,
    namespaces: Vec<NamespaceOutput>,
    tokens: Vec<TokenOutput>,
    grants: Vec<GrantOutput>,
    repos: i32,
}

pub fn run_info(data_dir: String, json: bool) -> anyhow::Result<()> {
    let store = init_store(&data_dir)?;

    let principals = store.list_principals("", 10000)?;
    let namespaces = store.list_namespaces("", 10000)?;
    let tokens = store.list_tokens("", 10000)?;

    let mut primary_count = 0;
    let mut repo_count = 0;

    for ns in &namespaces {
        if store.get_principal_by_primary_namespace_id(&ns.id)?.is_some() {
            primary_count += 1;
        }
        repo_count += store.list_repos(&ns.id, "", 10000)?.len() as i32;
    }

    let shared_count = namespaces.len() as i32 - primary_count;

    if json {
        let mut principal_outputs = Vec::with_capacity(principals.len());
        for principal in &principals {
            let username = store
                .get_namespace(&principal.primary_namespace_id)?
                .map(|ns| ns.name)
                .unwrap_or_else(|| "<unknown>".to_string());
            principal_outputs.push(PrincipalOutput {
                id: principal.id.clone(),
                username,
                created_at: principal.created_at.to_rfc3339(),
            });
        }

        let mut namespace_outputs = Vec::with_capacity(namespaces.len());
        for ns in &namespaces {
            let is_shared = store.get_principal_by_primary_namespace_id(&ns.id)?.is_none();
            namespace_outputs.push(NamespaceOutput {
                id: ns.id.clone(),
                name: ns.name.clone(),
                is_shared,
                created_at: ns.created_at.to_rfc3339(),
            });
        }

        let mut token_outputs = Vec::with_capacity(tokens.len());
        for token in &tokens {
            let username = if let Some(ref principal_id) = token.principal_id {
                if let Some(principal) = store.get_principal(principal_id)? {
                    store
                        .get_namespace(&principal.primary_namespace_id)?
                        .map(|ns| ns.name)
                } else {
                    None
                }
            } else {
                None
            };
            token_outputs.push(TokenOutput {
                id: token.id.clone(),
                lookup: token.token_lookup.clone(),
                principal_id: token.principal_id.clone(),
                username,
                is_admin: token.is_admin,
                created_at: token.created_at.to_rfc3339(),
                expires_at: token.expires_at.map(|dt| dt.to_rfc3339()),
                last_used_at: token.last_used_at.map(|dt| dt.to_rfc3339()),
            });
        }

        let mut grant_outputs = Vec::new();
        for principal in &principals {
            let username = store
                .get_namespace(&principal.primary_namespace_id)?
                .map(|ns| ns.name)
                .unwrap_or_else(|| "<unknown>".to_string());
            let grants = store.list_principal_namespace_grants(&principal.id)?;
            for grant in grants {
                let namespace_name = store
                    .get_namespace(&grant.namespace_id)?
                    .map(|ns| ns.name)
                    .unwrap_or_else(|| "<unknown>".to_string());
                grant_outputs.push(GrantOutput {
                    principal_id: principal.id.clone(),
                    username: username.clone(),
                    namespace_id: grant.namespace_id.clone(),
                    namespace_name,
                    permissions: grant.allow_bits.to_strings(),
                });
            }
        }

        let info = DetailedServerInfo {
            principals: principal_outputs,
            namespaces: namespace_outputs,
            tokens: token_outputs,
            grants: grant_outputs,
            repos: repo_count,
        };

        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        let info = ServerInfo {
            principals: principals.len() as i32,
            namespaces: namespaces.len() as i32,
            namespaces_primary: primary_count,
            namespaces_shared: shared_count,
            tokens: tokens.len() as i32,
            repos: repo_count,
        };

        println!();
        println!("Cutman Server Status");
        println!("{}", "─".repeat(20));
        println!("Principals:  {}", info.principals);
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
