use std::fmt;
use std::fmt::Write as _;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use sha2::{Digest, Sha256};
use url::Url;

use crate::ProviderId;

/// Secret-free identity for the exact provider, credential, endpoint, and profile that owns a
/// continuation.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct RuntimeProviderBindingIdentity {
    provider: ProviderId,
    credential_identity: String,
    endpoint_identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    profile_identity: Option<String>,
}

impl fmt::Debug for RuntimeProviderBindingIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProviderBindingIdentity")
            .field("provider", &self.provider)
            .field("credential_identity", &"<redacted>")
            .field("endpoint_identity", &"<redacted>")
            .field(
                "profile_identity",
                &self.profile_identity.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl RuntimeProviderBindingIdentity {
    pub fn from_raw_key(
        provider: ProviderId,
        raw_key: &str,
        endpoint: &str,
        profile: Option<&str>,
    ) -> Option<Self> {
        Self::from_public_credential_identity(provider, raw_key, endpoint, profile)
    }

    pub fn from_public_credential_identity(
        provider: ProviderId,
        credential_identity: &str,
        endpoint: &str,
        profile: Option<&str>,
    ) -> Option<Self> {
        let endpoint = canonical_endpoint(endpoint)?;
        let inputs = prodex_mojo_core::rich::provider_binding_identity_inputs(
            credential_identity,
            &endpoint,
            profile,
        )
        .expect("Mojo provider binding identity policy failed")?;
        Some(Self {
            provider,
            credential_identity: public_identity("credential", &inputs.credential),
            endpoint_identity: public_identity("endpoint", &inputs.endpoint),
            profile_identity: inputs
                .profile
                .as_deref()
                .map(|profile| public_identity("profile", profile)),
        })
    }

    pub fn from_profile(provider: ProviderId, profile: &str, endpoint: &str) -> Option<Self> {
        Self::from_public_credential_identity(provider, profile, endpoint, Some(profile))
    }

    pub fn provider(&self) -> ProviderId {
        self.provider
    }

    pub fn credential_identity(&self) -> &str {
        &self.credential_identity
    }

    pub fn endpoint_identity(&self) -> &str {
        &self.endpoint_identity
    }

    pub fn profile(&self) -> Option<&str> {
        self.profile_identity.as_deref()
    }

    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Deserialize)]
struct RuntimeProviderBindingIdentityWire {
    provider: ProviderId,
    credential_identity: String,
    endpoint_identity: String,
    #[serde(default)]
    profile_identity: Option<String>,
}

impl<'de> Deserialize<'de> for RuntimeProviderBindingIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RuntimeProviderBindingIdentityWire::deserialize(deserializer)?;
        let valid_identity = |value: &str| {
            prodex_mojo_core::rich::provider_binding_identity_digest_is_public(value)
                .map_err(|_| D::Error::custom("provider binding identity policy failed"))
        };
        if !valid_identity(&wire.credential_identity)?
            || !valid_identity(&wire.endpoint_identity)?
            || wire
                .profile_identity
                .as_deref()
                .map(valid_identity)
                .transpose()?
                .is_some_and(|valid| !valid)
        {
            return Err(D::Error::custom(
                "invalid runtime provider binding identity",
            ));
        }
        Ok(Self {
            provider: wire.provider,
            credential_identity: wire.credential_identity,
            endpoint_identity: wire.endpoint_identity,
            profile_identity: wire.profile_identity,
        })
    }
}

fn public_identity(kind: &str, value: &str) -> String {
    let mut input = Vec::with_capacity(kind.len() + value.len() + 32);
    input.extend_from_slice(b"prodex-runtime-binding-v1\0");
    input.extend_from_slice(kind.as_bytes());
    input.push(0);
    input.extend_from_slice(value.as_bytes());
    let digest = Sha256::digest(input);
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn canonical_endpoint(value: &str) -> Option<String> {
    let value = value.trim();
    let url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || value.len() > 2_048
    {
        return None;
    }
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_identity_round_trip_is_secret_and_endpoint_safe() {
        let identity = RuntimeProviderBindingIdentity::from_raw_key(
            ProviderId::OpenAi,
            "synthetic-key-material",
            "https://private.example.com/v1/",
            Some("synthetic-profile"),
        )
        .unwrap();
        let encoded = serde_json::to_string(&identity).unwrap();

        assert!(!encoded.contains("synthetic-key-material"));
        assert!(!encoded.contains("private.example.com"));
        assert!(!encoded.contains("synthetic-profile"));
        assert_eq!(
            serde_json::from_str::<RuntimeProviderBindingIdentity>(&encoded).unwrap(),
            identity
        );
        assert!(!format!("{identity:?}").contains("private.example.com"));
    }

    #[test]
    fn binding_identity_pins_key_endpoint_and_profile_independently() {
        let identity = |key, endpoint, profile| {
            RuntimeProviderBindingIdentity::from_raw_key(ProviderId::OpenAi, key, endpoint, profile)
                .unwrap()
        };
        let base = identity("key-one", "https://api.example.com/v1", Some("one"));
        assert_ne!(
            base,
            identity("key-two", "https://api.example.com/v1", Some("one"))
        );
        assert_ne!(
            base,
            identity("key-one", "https://other.example.com/v1", Some("one"))
        );
        assert_ne!(
            base,
            identity("key-one", "https://api.example.com/v1", Some("two"))
        );
    }

    #[test]
    fn binding_identity_rejects_raw_or_malformed_wire_values() {
        for value in [
            serde_json::json!({
                "provider": "openai",
                "credential_identity": "raw-key",
                "endpoint_identity": format!("sha256:{}", "a".repeat(64)),
            }),
            serde_json::json!({
                "provider": "openai",
                "credential_identity": format!("sha256:{}", "a".repeat(64)),
                "endpoint_identity": "https://api.example.com/v1",
            }),
        ] {
            assert!(serde_json::from_value::<RuntimeProviderBindingIdentity>(value).is_err());
        }
    }

    #[test]
    fn binding_identity_mojo_policy_preserves_unicode_and_boundary_inputs() {
        let identity = RuntimeProviderBindingIdentity::from_raw_key(
            ProviderId::OpenAi,
            "\u{2003}synthetic-key-🦀\u{3000}",
            "https://example.com/v1///",
            Some("\u{2003}synthetic-profile-雪\u{3000}"),
        )
        .unwrap();
        let expected = RuntimeProviderBindingIdentity::from_raw_key(
            ProviderId::OpenAi,
            "synthetic-key-🦀",
            "https://example.com/v1",
            Some("synthetic-profile-雪"),
        )
        .unwrap();
        assert_eq!(identity, expected);
        assert!(
            RuntimeProviderBindingIdentity::from_raw_key(
                ProviderId::OpenAi,
                &"x".repeat(4_097),
                "https://example.com",
                None,
            )
            .is_none()
        );
        assert!(
            RuntimeProviderBindingIdentity::from_raw_key(
                ProviderId::OpenAi,
                "credential",
                "https://example.com",
                Some(&"x".repeat(256)),
            )
            .is_some()
        );
        assert!(
            RuntimeProviderBindingIdentity::from_raw_key(
                ProviderId::OpenAi,
                "credential",
                "https://example.com",
                Some(&"x".repeat(257)),
            )
            .is_none()
        );
    }

    #[test]
    fn binding_identity_mojo_boundary_returns_the_normalized_plan() {
        let plan = prodex_mojo_core::rich::provider_binding_identity_inputs(
            "\u{2003}credential\u{3000}",
            "https://example.com/v1///",
            Some("\u{2003}profile\u{3000}"),
        )
        .expect("Mojo binding identity boundary should be available")
        .expect("valid input should be accepted");
        assert_eq!(plan.credential, "credential");
        assert_eq!(plan.endpoint, "https://example.com/v1");
        assert_eq!(plan.profile.as_deref(), Some("profile"));
        assert!(
            prodex_mojo_core::rich::provider_binding_identity_digest_is_public(&format!(
                "sha256:{}",
                "a".repeat(64)
            ))
            .expect("Mojo digest boundary should be available")
        );
    }
}
