use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use rand::Rng;

use crate::error::{Error, Result};

const ARGON2_MEMORY: u32 = 64 * 1024; // 64KB
const ARGON2_ITERATIONS: u32 = 1;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_OUTPUT_LEN: usize = 32;

const TOKEN_PREFIX: &str = "cutman";
const LOOKUP_LENGTH: usize = 8;
const SECRET_LENGTH: usize = 24;
const SECRET_BYTES: usize = 12;

pub struct TokenGenerator {
    argon2: Argon2<'static>,
}

impl Default for TokenGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenGenerator {
    #[must_use]
    pub fn new() -> Self {
        let params = Params::new(
            ARGON2_MEMORY,
            ARGON2_ITERATIONS,
            ARGON2_PARALLELISM,
            Some(ARGON2_OUTPUT_LEN),
        )
        .expect("invalid argon2 params");

        Self {
            argon2: Argon2::new(Algorithm::Argon2id, Version::V0x13, params),
        }
    }

    /// Generates a new token with the format: cutman_<lookup>_<secret>
    /// Returns (raw_token, lookup, hash)
    pub fn generate(&self) -> Result<(String, String, String)> {
        let lookup = generate_lookup();
        let secret = generate_secret();
        let raw_token = build_token(&lookup, &secret);
        let hash = self.hash(&raw_token)?;
        Ok((raw_token, lookup, hash))
    }

    /// Hashes a raw token using Argon2id
    pub fn hash(&self, token: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon2
            .hash_password(token.as_bytes(), &salt)
            .map_err(|e| Error::Config(format!("failed to hash token: {e}")))?;
        Ok(hash.to_string())
    }

    /// Verifies a raw token against a stored hash
    pub fn verify(&self, token: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| Error::Config(format!("invalid hash format: {e}")))?;

        match self.argon2.verify_password(token.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(Error::Config(format!("failed to verify token: {e}"))),
        }
    }
}

/// Generates the lookup portion of the token (first 8 chars of a UUID)
#[must_use]
fn generate_lookup() -> String {
    let uuid = uuid::Uuid::new_v4();
    uuid.to_string()[..LOOKUP_LENGTH].to_string()
}

/// Generates a cryptographically secure random hex string for the secret
#[must_use]
fn generate_secret() -> String {
    let mut bytes = [0u8; SECRET_BYTES];
    rand::thread_rng().fill(&mut bytes);
    hex::encode(&bytes)[..SECRET_LENGTH].to_string()
}

/// Builds the full token string from lookup and secret
#[must_use]
fn build_token(lookup: &str, secret: &str) -> String {
    format!("{TOKEN_PREFIX}_{lookup}_{secret}")
}

/// Parses a token string into its components (lookup, secret)
pub fn parse_token(token: &str) -> Result<(String, String)> {
    let prefix = format!("{TOKEN_PREFIX}_");
    if !token.starts_with(&prefix) {
        return Err(Error::InvalidTokenFormat);
    }

    let parts: Vec<&str> = token.split('_').collect();
    if parts.len() != 3 {
        return Err(Error::InvalidTokenFormat);
    }

    let lookup = parts[1];
    let secret = parts[2];

    if lookup.len() != LOOKUP_LENGTH || secret.len() != SECRET_LENGTH {
        return Err(Error::InvalidTokenFormat);
    }

    Ok((lookup.to_string(), secret.to_string()))
}

// Re-export hex for use in token generation
mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_format() {
        let generator = TokenGenerator::new();
        let (token, lookup, _hash) = generator.generate().unwrap();

        assert!(token.starts_with("cutman_"));
        assert_eq!(lookup.len(), 8);

        let parts: Vec<&str> = token.split('_').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "cutman");
        assert_eq!(parts[1].len(), 8);
        assert_eq!(parts[2].len(), 24);
    }

    #[test]
    fn test_token_verification_correct() {
        let generator = TokenGenerator::new();
        let (token, _, hash) = generator.generate().unwrap();

        assert!(generator.verify(&token, &hash).unwrap());
    }

    #[test]
    fn test_token_verification_wrong_secret() {
        let generator = TokenGenerator::new();
        let (token, _, hash) = generator.generate().unwrap();

        let wrong_token = format!("{}_wrong", &token[..token.len() - 5]);
        assert!(!generator.verify(&wrong_token, &hash).unwrap());
    }

    #[test]
    fn test_parse_token_valid() {
        let (lookup, secret) = parse_token("cutman_12345678_123456789012345678901234").unwrap();
        assert_eq!(lookup, "12345678");
        assert_eq!(secret, "123456789012345678901234");
    }

    #[test]
    fn test_parse_token_invalid_prefix() {
        let result = parse_token("invalid_12345678_123456789012345678901234");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_token_wrong_parts() {
        let result = parse_token("cutman_12345678");
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_is_phc_format() {
        let generator = TokenGenerator::new();
        let (_, _, hash) = generator.generate().unwrap();

        assert!(hash.starts_with("$argon2id$"));
    }
}
