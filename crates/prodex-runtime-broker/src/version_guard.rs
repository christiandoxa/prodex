use super::*;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBrokerVersionGuardOutcome {
    Compatible,
    Replaced,
    DeferredActiveRequests,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBrokerVersionGuardDecision {
    pub outcome: RuntimeBrokerVersionGuardOutcome,
    pub current_identity: RuntimeProdexBinaryIdentity,
    pub replacement_reason: Option<&'static str>,
}

pub fn runtime_broker_key_for_binary_identity(
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    smart_context_enabled: bool,
    model_context_window_tokens: Option<u64>,
    openai_mount_path: &str,
    binary_identity_key: &str,
) -> String {
    let mut hasher = DefaultHasher::new();
    let model_context_window_tokens = smart_context_enabled
        .then_some(model_context_window_tokens)
        .flatten();
    upstream_base_url.hash(&mut hasher);
    include_code_review.hash(&mut hasher);
    upstream_no_proxy.hash(&mut hasher);
    smart_context_enabled.hash(&mut hasher);
    model_context_window_tokens.hash(&mut hasher);
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

pub fn runtime_broker_observed_known_binary_identity(
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
) -> Option<RuntimeProdexBinaryIdentity> {
    health
        .filter(|health| health.matches_registry_instance(registry))
        .map(runtime_health_prodex_binary_identity)
        .filter(RuntimeProdexBinaryIdentity::is_present)
        .or_else(|| {
            let identity = runtime_registry_prodex_binary_identity(registry);
            identity.is_present().then_some(identity)
        })
}

pub fn runtime_broker_observed_binary_identity(
    registry: &RuntimeBrokerRegistry,
    health: Option<&RuntimeBrokerHealth>,
    process_identity: Option<&RuntimeProdexBinaryIdentity>,
) -> RuntimeProdexBinaryIdentity {
    runtime_broker_observed_known_binary_identity(registry, health)
        .or_else(|| {
            process_identity
                .filter(|identity| identity.is_present())
                .cloned()
        })
        .unwrap_or_default()
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

pub fn runtime_broker_replacement_reason(
    current: &RuntimeProdexBinaryIdentity,
    observed: &RuntimeProdexBinaryIdentity,
) -> &'static str {
    match (
        current.executable_sha256.as_deref(),
        observed.executable_sha256.as_deref(),
    ) {
        (Some(current_sha256), Some(observed_sha256)) if current_sha256 != observed_sha256 => {
            "sha256_mismatch"
        }
        _ => match (
            current.prodex_version.as_deref(),
            observed.prodex_version.as_deref(),
        ) {
            (Some(current_version), Some(observed_version))
                if current_version != observed_version =>
            {
                "version_mismatch"
            }
            _ if observed.is_present() => "identity_mismatch",
            _ => "identity_unresolved",
        },
    }
}

pub fn runtime_broker_observed_version_mismatch(
    current_version_identity: &RuntimeProdexBinaryIdentity,
    observed_identity: &RuntimeProdexBinaryIdentity,
) -> bool {
    observed_identity
        .prodex_version
        .as_deref()
        .zip(current_version_identity.prodex_version.as_deref())
        .is_some_and(|(observed, current)| observed != current)
}

pub fn runtime_broker_version_guard_decision(
    process_alive: bool,
    current_binary_identity: &RuntimeProdexBinaryIdentity,
    current_version_identity: &RuntimeProdexBinaryIdentity,
    observed_identity: &RuntimeProdexBinaryIdentity,
    active_requests: usize,
    live_leases: usize,
) -> RuntimeBrokerVersionGuardDecision {
    let current_identity =
        if runtime_broker_observed_version_mismatch(current_version_identity, observed_identity) {
            current_version_identity.clone()
        } else {
            current_binary_identity.clone()
        };

    if !process_alive
        || (observed_identity.is_present()
            && runtime_prodex_binary_identity_matches(&current_identity, observed_identity))
    {
        return RuntimeBrokerVersionGuardDecision {
            outcome: RuntimeBrokerVersionGuardOutcome::Compatible,
            current_identity,
            replacement_reason: None,
        };
    }

    if active_requests > 0 || live_leases > 0 {
        return RuntimeBrokerVersionGuardDecision {
            outcome: RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests,
            current_identity,
            replacement_reason: None,
        };
    }

    let replacement_reason =
        runtime_broker_replacement_reason(&current_identity, observed_identity);
    RuntimeBrokerVersionGuardDecision {
        outcome: RuntimeBrokerVersionGuardOutcome::Replaced,
        current_identity,
        replacement_reason: Some(replacement_reason),
    }
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
