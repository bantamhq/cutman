use std::fmt;

use serde::{Deserialize, Serialize};

/// Permission represents a bitmask of granted permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Permission(u32);

impl Permission {
    pub const REPO_READ: Permission = Permission(1 << 0); // 1
    pub const REPO_WRITE: Permission = Permission(1 << 1); // 2
    pub const REPO_ADMIN: Permission = Permission(1 << 2); // 4
    pub const NAMESPACE_READ: Permission = Permission(1 << 3); // 8
    pub const NAMESPACE_WRITE: Permission = Permission(1 << 4); // 16
    pub const NAMESPACE_ADMIN: Permission = Permission(1 << 5); // 32

    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns true if this permission bitmask contains the required permission.
    #[must_use]
    pub const fn has(self, required: Permission) -> bool {
        self.0 & required.0 == required.0
    }

    /// Combines two permission bitmasks.
    #[must_use]
    pub const fn union(self, other: Permission) -> Permission {
        Permission(self.0 | other.0)
    }

    /// Removes permissions from this bitmask.
    #[must_use]
    pub const fn difference(self, other: Permission) -> Permission {
        Permission(self.0 & !other.0)
    }

    /// Expands a permission bitmask to include implied permissions.
    /// admin implies write implies read, for both repo and namespace permissions.
    /// This should only be used for ALLOW permissions, never for DENY.
    #[must_use]
    pub fn expand_implied(self) -> Permission {
        let mut result = self.0;

        if self.has(Self::REPO_ADMIN) {
            result |= Self::REPO_WRITE.0;
        }
        if Permission(result).has(Self::REPO_WRITE) {
            result |= Self::REPO_READ.0;
        }

        if self.has(Self::NAMESPACE_ADMIN) {
            result |= Self::NAMESPACE_WRITE.0;
        }
        if Permission(result).has(Self::NAMESPACE_WRITE) {
            result |= Self::NAMESPACE_READ.0;
        }

        Permission(result)
    }

    /// Returns the default permissions for simple token creation:
    /// namespace:write + repo:admin (which implies namespace:read, repo:read, repo:write).
    #[must_use]
    pub const fn default_namespace_grant() -> Permission {
        Permission(Self::NAMESPACE_WRITE.0 | Self::REPO_ADMIN.0)
    }

    /// Converts a permission string to its bitmask value.
    pub fn parse(s: &str) -> Option<Permission> {
        match s {
            "repo:read" => Some(Self::REPO_READ),
            "repo:write" => Some(Self::REPO_WRITE),
            "repo:admin" => Some(Self::REPO_ADMIN),
            "namespace:read" => Some(Self::NAMESPACE_READ),
            "namespace:write" => Some(Self::NAMESPACE_WRITE),
            "namespace:admin" => Some(Self::NAMESPACE_ADMIN),
            _ => None,
        }
    }

    /// Converts a slice of permission strings to a combined bitmask.
    pub fn parse_many(strs: &[&str]) -> Option<Permission> {
        let mut result = Permission::default();
        for s in strs {
            result = result.union(Self::parse(s)?);
        }
        Some(result)
    }

    /// Returns a slice of permission strings for this bitmask.
    #[must_use]
    pub fn to_strings(self) -> Vec<&'static str> {
        let mut perms = Vec::new();
        if self.has(Self::REPO_READ) {
            perms.push("repo:read");
        }
        if self.has(Self::REPO_WRITE) {
            perms.push("repo:write");
        }
        if self.has(Self::REPO_ADMIN) {
            perms.push("repo:admin");
        }
        if self.has(Self::NAMESPACE_READ) {
            perms.push("namespace:read");
        }
        if self.has(Self::NAMESPACE_WRITE) {
            perms.push("namespace:write");
        }
        if self.has(Self::NAMESPACE_ADMIN) {
            perms.push("namespace:admin");
        }
        perms
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_strings().join(", "))
    }
}

impl From<u32> for Permission {
    fn from(bits: u32) -> Self {
        Self(bits)
    }
}

impl From<Permission> for u32 {
    fn from(p: Permission) -> Self {
        p.0
    }
}

impl From<i64> for Permission {
    fn from(bits: i64) -> Self {
        Self(bits as u32)
    }
}

impl From<Permission> for i64 {
    fn from(p: Permission) -> Self {
        p.0 as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_has() {
        let p = Permission::REPO_READ.union(Permission::REPO_WRITE);
        assert!(p.has(Permission::REPO_READ));
        assert!(p.has(Permission::REPO_WRITE));
        assert!(!p.has(Permission::REPO_ADMIN));
    }

    #[test]
    fn test_expand_implied() {
        let admin = Permission::REPO_ADMIN;
        let expanded = admin.expand_implied();
        assert!(expanded.has(Permission::REPO_ADMIN));
        assert!(expanded.has(Permission::REPO_WRITE));
        assert!(expanded.has(Permission::REPO_READ));
    }

    #[test]
    fn test_parse_permission() {
        assert_eq!(Permission::parse("repo:read"), Some(Permission::REPO_READ));
        assert_eq!(Permission::parse("invalid"), None);
    }
}
