use super::smart_context_sha256_digest;
use serde::{Deserialize, Serialize};
use std::fmt;

const SMART_CONTEXT_SCOPE_PREFIX: &str = "scscope1:";

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContextScopeId(String);

impl ContextScopeId {
    pub fn new(
        tenant: &str,
        profile: &str,
        provider: &str,
        workspace: &str,
        session: Option<&str>,
    ) -> Self {
        let mut bytes = b"prodex-smart-context-scope-v1\0".to_vec();
        for value in [tenant, profile, provider, workspace, session.unwrap_or("")] {
            bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
            bytes.extend_from_slice(value.as_bytes());
        }
        let digest = smart_context_sha256_digest(&bytes);
        Self(format!(
            "{SMART_CONTEXT_SCOPE_PREFIX}{}",
            hex_digest(&digest)
        ))
    }

    pub fn parse(value: &str) -> Option<Self> {
        let digest = value.strip_prefix(SMART_CONTEXT_SCOPE_PREFIX)?;
        (digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| Self(value.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn path_component(&self) -> &str {
        self.0
            .strip_prefix(SMART_CONTEXT_SCOPE_PREFIX)
            .unwrap_or(&self.0)
    }
}

impl fmt::Debug for ContextScopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ContextScopeId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for ContextScopeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_is_stable_private_and_separates_every_boundary() {
        let base = ContextScopeId::new(
            "tenant-a",
            "profile-a@example.com",
            "openai",
            "/home/test-user/repo",
            Some("session-a"),
        );
        assert_eq!(ContextScopeId::parse(base.as_str()).as_ref(), Some(&base));
        for changed in [
            ContextScopeId::new(
                "tenant-b",
                "profile-a@example.com",
                "openai",
                "/home/test-user/repo",
                Some("session-a"),
            ),
            ContextScopeId::new(
                "tenant-a",
                "profile-b@example.com",
                "openai",
                "/home/test-user/repo",
                Some("session-a"),
            ),
            ContextScopeId::new(
                "tenant-a",
                "profile-a@example.com",
                "anthropic",
                "/home/test-user/repo",
                Some("session-a"),
            ),
            ContextScopeId::new(
                "tenant-a",
                "profile-a@example.com",
                "openai",
                "/home/test-user/other",
                Some("session-a"),
            ),
            ContextScopeId::new(
                "tenant-a",
                "profile-a@example.com",
                "openai",
                "/home/test-user/repo",
                Some("session-b"),
            ),
        ] {
            assert_ne!(base, changed);
        }
        for private in ["tenant-a", "profile-a", "test-user", "session-a"] {
            assert!(!base.as_str().contains(private));
        }
    }
}
