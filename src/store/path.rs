use crate::error::{Error, Result};

pub fn normalize_path(path: &str) -> Result<String> {
    let path = path.trim();

    if path.is_empty() {
        return Err(Error::BadRequest("Path cannot be empty".to_string()));
    }

    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if segments.is_empty() {
        return Err(Error::BadRequest("Path cannot be empty".to_string()));
    }

    for segment in &segments {
        validate_segment(segment)?;
    }

    Ok(format!("/{}", segments.join("/")))
}

fn validate_segment(segment: &str) -> Result<()> {
    if segment.is_empty() {
        return Err(Error::BadRequest(
            "Path segment cannot be empty".to_string(),
        ));
    }

    if segment.len() > 255 {
        return Err(Error::BadRequest(
            "Path segment cannot exceed 255 characters".to_string(),
        ));
    }

    const INVALID_CHARS: &[char] = &['\0', '\n', '\r'];
    if segment.chars().any(|c| INVALID_CHARS.contains(&c)) {
        return Err(Error::BadRequest(
            "Path segment contains invalid characters".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_basic() {
        assert_eq!(normalize_path("engineering").unwrap(), "/engineering");
        assert_eq!(normalize_path("/engineering").unwrap(), "/engineering");
        assert_eq!(normalize_path("engineering/").unwrap(), "/engineering");
        assert_eq!(normalize_path("/engineering/").unwrap(), "/engineering");
    }

    #[test]
    fn test_normalize_path_nested() {
        assert_eq!(
            normalize_path("engineering/backend").unwrap(),
            "/engineering/backend"
        );
        assert_eq!(
            normalize_path("/engineering/backend/").unwrap(),
            "/engineering/backend"
        );
    }

    #[test]
    fn test_normalize_path_collapses_slashes() {
        assert_eq!(
            normalize_path("//engineering//backend//").unwrap(),
            "/engineering/backend"
        );
    }

    #[test]
    fn test_normalize_path_empty_error() {
        assert!(normalize_path("").is_err());
        assert!(normalize_path("/").is_err());
        assert!(normalize_path("//").is_err());
    }
}
