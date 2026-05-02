//! Runtime broker registry, health, and metrics transfer models.
//!
//! This crate intentionally stays side-effect-free. The binary crate still owns
//! broker leases, proxy handles, and registry file I/O.

use prodex_runtime_state::RuntimeStateLockWaitMetrics;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerRegistry {
    pub pid: u32,
    pub listen_addr: String,
    pub started_at: i64,
    pub upstream_base_url: String,
    pub include_code_review: bool,
    #[serde(default)]
    pub upstream_no_proxy: bool,
    pub current_profile: String,
    pub instance_token: String,
    pub admin_token: String,
    #[serde(default)]
    pub prodex_version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
    #[serde(default)]
    pub executable_sha256: Option<String>,
    #[serde(default)]
    pub openai_mount_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerHealth {
    pub pid: u32,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub active_requests: usize,
    pub instance_token: String,
    pub persistence_role: String,
    #[serde(default)]
    pub prodex_version: Option<String>,
    #[serde(default)]
    pub executable_path: Option<String>,
    #[serde(default)]
    pub executable_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerLaneMetrics {
    pub active: usize,
    pub limit: usize,
    pub admissions_total: u64,
    pub global_limit_rejections_total: u64,
    pub lane_limit_rejections_total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerTrafficMetrics {
    pub responses: RuntimeBrokerLaneMetrics,
    pub compact: RuntimeBrokerLaneMetrics,
    pub websocket: RuntimeBrokerLaneMetrics,
    pub standard: RuntimeBrokerLaneMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuationSignalMetrics {
    pub response: usize,
    pub turn_state: usize,
    pub session_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuityFailureReasonMetrics {
    #[serde(default)]
    pub chain_retried_owner: BTreeMap<String, usize>,
    #[serde(default)]
    pub chain_dead_upstream_confirmed: BTreeMap<String, usize>,
    #[serde(default)]
    pub stale_continuation: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerRouteContinuityMetrics {
    pub responses: usize,
    pub compact: usize,
    pub websocket: usize,
    pub standard: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeBrokerPreviousResponseContinuityMetrics {
    pub negative_cache_entries: RuntimeBrokerRouteContinuityMetrics,
    pub negative_cache_failures: RuntimeBrokerRouteContinuityMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerContinuationMetrics {
    pub response_bindings: usize,
    pub turn_state_bindings: usize,
    pub session_id_bindings: usize,
    pub warm: usize,
    pub verified: usize,
    pub suspect: usize,
    pub dead: usize,
    #[serde(default)]
    pub failure_counts: RuntimeBrokerContinuationSignalMetrics,
    #[serde(default)]
    pub not_found_streaks: RuntimeBrokerContinuationSignalMetrics,
    #[serde(default)]
    pub stale_verified_bindings: RuntimeBrokerContinuationSignalMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBrokerMetrics {
    pub health: RuntimeBrokerHealth,
    pub active_request_limit: usize,
    pub local_overload_backoff_remaining_seconds: u64,
    #[serde(default)]
    pub runtime_state_lock_wait: RuntimeStateLockWaitMetrics,
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: BTreeMap<String, usize>,
    pub retry_backoffs: usize,
    pub transport_backoffs: usize,
    pub route_circuits: usize,
    pub degraded_profiles: usize,
    pub degraded_routes: usize,
    pub continuations: RuntimeBrokerContinuationMetrics,
    #[serde(default)]
    pub previous_response_continuity: RuntimeBrokerPreviousResponseContinuityMetrics,
    #[serde(default)]
    pub continuity_failure_reasons: RuntimeBrokerContinuityFailureReasonMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeBrokerObservation {
    pub broker_key: String,
    pub listen_addr: String,
    pub metrics: RuntimeBrokerMetrics,
}

#[derive(Debug, Clone)]
pub struct RuntimeBrokerMetadata {
    pub broker_key: String,
    pub listen_addr: String,
    pub started_at: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub upstream_no_proxy: bool,
    pub instance_token: String,
    pub admin_token: String,
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProdexBinaryIdentity {
    pub prodex_version: Option<String>,
    pub executable_path: Option<PathBuf>,
    pub executable_sha256: Option<String>,
}

impl RuntimeProdexBinaryIdentity {
    pub fn is_present(&self) -> bool {
        self.prodex_version.is_some()
            || self.executable_path.is_some()
            || self.executable_sha256.is_some()
    }
}

pub fn runtime_broker_key_for_binary_identity(
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    openai_mount_path: &str,
    binary_identity_key: &str,
) -> String {
    let mut hasher = DefaultHasher::new();
    upstream_base_url.hash(&mut hasher);
    include_code_review.hash(&mut hasher);
    upstream_no_proxy.hash(&mut hasher);
    openai_mount_path.hash(&mut hasher);
    binary_identity_key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn runtime_prodex_binary_identity_key(identity: &RuntimeProdexBinaryIdentity) -> String {
    match (
        identity.prodex_version.as_deref(),
        identity.executable_sha256.as_deref(),
        identity.executable_path.as_ref(),
    ) {
        (Some(version), Some(sha256), _) => format!("version={version};sha256={sha256}"),
        (Some(version), None, Some(path)) => {
            format!("version={version};path={}", path.display())
        }
        (Some(version), None, None) => format!("version={version}"),
        (None, Some(sha256), _) => format!("sha256={sha256}"),
        (None, None, Some(path)) => format!("path={}", path.display()),
        (None, None, None) => "unknown".to_string(),
    }
}

pub fn runtime_registry_prodex_binary_identity(
    registry: &RuntimeBrokerRegistry,
) -> RuntimeProdexBinaryIdentity {
    RuntimeProdexBinaryIdentity {
        prodex_version: registry.prodex_version.clone(),
        executable_path: registry.executable_path.clone().map(PathBuf::from),
        executable_sha256: registry.executable_sha256.clone(),
    }
}

pub fn runtime_health_prodex_binary_identity(
    health: &RuntimeBrokerHealth,
) -> RuntimeProdexBinaryIdentity {
    RuntimeProdexBinaryIdentity {
        prodex_version: health.prodex_version.clone(),
        executable_path: health.executable_path.clone().map(PathBuf::from),
        executable_sha256: health.executable_sha256.clone(),
    }
}

pub fn runtime_prodex_binary_identity_matches(
    current: &RuntimeProdexBinaryIdentity,
    other: &RuntimeProdexBinaryIdentity,
) -> bool {
    if let (Some(current_sha256), Some(other_sha256)) = (
        current.executable_sha256.as_deref(),
        other.executable_sha256.as_deref(),
    ) {
        return current_sha256 == other_sha256;
    }
    if let (Some(current_version), Some(other_version)) = (
        current.prodex_version.as_deref(),
        other.prodex_version.as_deref(),
    ) {
        return current_version == other_version;
    }
    false
}

pub fn parse_prodex_version_output(output: &str) -> Option<String> {
    let mut parts = output.split_whitespace();
    let binary_name = parts.next()?;
    let version = parts.next()?;
    if binary_name == "prodex" && !version.is_empty() {
        return Some(version.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prodex_binary_identity_key_prefers_version_and_sha() {
        let identity = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: Some(PathBuf::from("/tmp/prodex")),
            executable_sha256: Some("abc123".to_string()),
        };

        assert_eq!(
            runtime_prodex_binary_identity_key(&identity),
            "version=0.7.0;sha256=abc123"
        );
    }

    #[test]
    fn prodex_binary_identity_match_prefers_sha_then_version() {
        let current = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: None,
            executable_sha256: Some("abc123".to_string()),
        };
        let other_same_sha = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.8.0".to_string()),
            executable_path: None,
            executable_sha256: Some("abc123".to_string()),
        };
        let other_same_version = RuntimeProdexBinaryIdentity {
            prodex_version: Some("0.7.0".to_string()),
            executable_path: None,
            executable_sha256: None,
        };

        assert!(runtime_prodex_binary_identity_matches(
            &current,
            &other_same_sha
        ));
        assert!(runtime_prodex_binary_identity_matches(
            &current,
            &other_same_version
        ));
    }

    #[test]
    fn parse_prodex_version_output_requires_prodex_binary_name() {
        assert_eq!(
            parse_prodex_version_output("prodex 0.7.0\n"),
            Some("0.7.0".to_string())
        );
        assert_eq!(parse_prodex_version_output("codex 0.7.0\n"), None);
        assert_eq!(parse_prodex_version_output("prodex\n"), None);
    }
}
