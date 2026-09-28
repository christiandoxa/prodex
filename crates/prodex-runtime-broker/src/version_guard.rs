use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeProdexBinaryIdentity {
    pub prodex_version: Option<String>,
    pub executable_path: Option<PathBuf>,
    pub executable_sha256: Option<String>,
}

fn mojo_binary_identity(
    identity: &RuntimeProdexBinaryIdentity,
) -> prodex_mojo_core::runtime_broker_continuity::BrokerBinaryIdentityView<'_> {
    prodex_mojo_core::runtime_broker_continuity::BrokerBinaryIdentityView {
        version: identity.prodex_version.as_deref(),
        sha256: identity.executable_sha256.as_deref(),
        path_present: identity.executable_path.is_some(),
    }
}

fn mojo_replacement_reason(
    reason: prodex_mojo_core::runtime_broker_continuity::BrokerReplacementReason,
) -> &'static str {
    use prodex_mojo_core::runtime_broker_continuity::BrokerReplacementReason;
    match reason {
        BrokerReplacementReason::Sha256Mismatch => "sha256_mismatch",
        BrokerReplacementReason::VersionMismatch => "version_mismatch",
        BrokerReplacementReason::IdentityMismatch => "identity_mismatch",
        BrokerReplacementReason::IdentityUnresolved => "identity_unresolved",
    }
}

impl RuntimeProdexBinaryIdentity {
    pub fn is_present(&self) -> bool {
        prodex_mojo_core::runtime_broker_continuity::binary_identity_present(mojo_binary_identity(
            self,
        ))
        .expect("Mojo runtime broker identity-presence policy returned invalid output")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBrokerVersionGuardOutcome {
    Compatible,
    Replaced,
    DeferredActiveRequests,
    TerminationFailed,
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
    prodex_mojo_core::runtime_broker_continuity::binary_identity_matches(
        mojo_binary_identity(current),
        mojo_binary_identity(other),
    )
    .expect("Mojo runtime broker identity-match policy returned invalid output")
}

pub fn runtime_broker_replacement_reason(
    current: &RuntimeProdexBinaryIdentity,
    observed: &RuntimeProdexBinaryIdentity,
) -> &'static str {
    let reason = prodex_mojo_core::runtime_broker_continuity::binary_identity_replacement_reason(
        mojo_binary_identity(current),
        mojo_binary_identity(observed),
    )
    .expect("Mojo runtime broker replacement-reason policy returned invalid output");
    mojo_replacement_reason(reason)
}

pub fn runtime_broker_observed_version_mismatch(
    current_version_identity: &RuntimeProdexBinaryIdentity,
    observed_identity: &RuntimeProdexBinaryIdentity,
) -> bool {
    prodex_mojo_core::runtime_broker_continuity::binary_identity_version_mismatch(
        mojo_binary_identity(current_version_identity),
        mojo_binary_identity(observed_identity),
    )
    .expect("Mojo runtime broker version-mismatch policy returned invalid output")
}

pub fn runtime_broker_version_guard_decision(
    process_alive: bool,
    current_binary_identity: &RuntimeProdexBinaryIdentity,
    current_version_identity: &RuntimeProdexBinaryIdentity,
    observed_identity: &RuntimeProdexBinaryIdentity,
    active_requests: usize,
    live_leases: usize,
) -> RuntimeBrokerVersionGuardDecision {
    use prodex_mojo_core::runtime_broker_continuity::BrokerVersionGuardOutcome;

    let plan = prodex_mojo_core::runtime_broker_continuity::version_guard_plan(
        process_alive,
        mojo_binary_identity(current_binary_identity),
        mojo_binary_identity(current_version_identity),
        mojo_binary_identity(observed_identity),
        active_requests,
        live_leases,
    )
    .expect("Mojo runtime broker version-guard policy returned invalid output");

    let current_identity = if plan.use_version_identity {
        current_version_identity.clone()
    } else {
        current_binary_identity.clone()
    };
    let outcome = match plan.outcome {
        BrokerVersionGuardOutcome::Compatible => RuntimeBrokerVersionGuardOutcome::Compatible,
        BrokerVersionGuardOutcome::DeferredActiveRequests => {
            RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests
        }
        BrokerVersionGuardOutcome::Replaced => RuntimeBrokerVersionGuardOutcome::Replaced,
    };
    RuntimeBrokerVersionGuardDecision {
        outcome,
        current_identity,
        replacement_reason: plan.replacement_reason.map(mojo_replacement_reason),
    }
}

pub fn parse_prodex_version_output(output: &str) -> Option<String> {
    prodex_mojo_core::runtime_broker_continuity::parse_prodex_version(output)
        .expect("Mojo runtime broker version-output parser returned invalid output")
        .map(str::to_string)
}
