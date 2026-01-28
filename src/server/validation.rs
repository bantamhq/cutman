use crate::server::response::ApiError;

const MAX_NAMESPACE_NAME_LEN: usize = 64;
const MAX_REPO_NAME_LEN: usize = 100;

fn is_valid_name_char(c: char, allow_period: bool) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || (allow_period && c == '.')
}

fn validate_name(
    name: &str,
    entity: &str,
    max_len: usize,
    allow_period: bool,
    forbid_leading_special: bool,
) -> Result<(), String> {
    if name.is_empty() {
        return Err(format!("{entity} name cannot be empty"));
    }
    if name.len() > max_len {
        return Err(format!("{entity} name cannot exceed {max_len} characters"));
    }
    if !name.chars().all(|c| is_valid_name_char(c, allow_period)) {
        let mut allowed = "alphanumeric characters, hyphens, and underscores".to_string();
        if allow_period {
            allowed.push_str(", and periods");
        }
        return Err(format!("{entity} name can only contain {allowed}"));
    }
    if forbid_leading_special && (name.starts_with('-') || name.starts_with('_')) {
        return Err(format!(
            "{entity} name cannot start with a hyphen or underscore"
        ));
    }
    Ok(())
}

pub fn validate_namespace_name(name: &str) -> Result<(), String> {
    validate_name(name, "Namespace", MAX_NAMESPACE_NAME_LEN, false, true)
}

pub fn validate_repo_name(name: &str) -> Result<(), ApiError> {
    validate_name(name, "Repository", MAX_REPO_NAME_LEN, true, false).map_err(ApiError::bad_request)
}
