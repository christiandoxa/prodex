use crate::types::{
    PRODEX_POLICY_VERSION, RuntimeGovernanceDataClassification, RuntimeGovernanceMode,
    RuntimeGovernancePolicyFailureMode, RuntimeGovernanceRolloutMode,
    RuntimeGovernanceUnknownClassificationBehavior, RuntimePolicyFile,
};
use crate::validate_secrets::validate_secret_policy;
use anyhow::{Result, bail};
use std::path::Path;

pub const MAX_GOVERNANCE_INSPECTION_PATTERNS: usize = 64;
pub const MAX_GOVERNANCE_INSPECTION_PATTERNS_PER_TENANT: usize = 16;
pub const MAX_GOVERNANCE_INSPECTION_PATTERN_ID_BYTES: usize = 64;
pub const MAX_GOVERNANCE_INSPECTION_PATTERN_BYTES: usize = 256;
pub const MAX_GOVERNANCE_INSPECTION_PATTERN_WILDCARDS: usize = 8;
pub const MAX_GOVERNANCE_AUTHORITY_TENANTS: usize = 64;
const MIN_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS: u32 = 300;
const MAX_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS: u32 = 86_400;
const MIN_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS: u32 = 60;
const MAX_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS: u32 = 3_600;
const MAX_GOVERNANCE_SESSION_CONCURRENT: u32 = 10_000;

#[path = "validate/errors.rs"]
mod errors;
#[path = "validate/gateway/mod.rs"]
mod gateway;
#[path = "validate/runtime.rs"]
mod runtime;
#[path = "validate/runtime_proxy.rs"]
mod runtime_proxy;
#[path = "validate/service_mode.rs"]
mod service_mode;

pub use crate::validate_secrets::parse_secret_backend_kind;
pub use errors::{
    RuntimePolicyValidationErrors, RuntimePolicyValidationIssue, RuntimePolicyValidationSection,
};
pub use runtime_proxy::validate_runtime_proxy_policy;

pub fn validate_runtime_policy_file(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let mut errors = RuntimePolicyValidationErrors::default();
    errors.capture(
        RuntimePolicyValidationSection::Version,
        validate_policy_version(policy, path),
    );
    errors.capture(
        RuntimePolicyValidationSection::ServiceMode,
        service_mode::validate_service_mode(policy, path),
    );
    errors.capture(
        RuntimePolicyValidationSection::Runtime,
        runtime::validate_runtime_policy(policy, path),
    );
    errors.capture(
        RuntimePolicyValidationSection::Secrets,
        validate_secret_policy(policy, path),
    );
    errors.capture(
        RuntimePolicyValidationSection::RuntimeProxy,
        validate_runtime_proxy_policy(policy, path),
    );
    errors.capture(
        RuntimePolicyValidationSection::Gateway,
        validate_governance_policy(policy, path),
    );
    gateway::collect_gateway_policy_errors(&mut errors, policy, path);
    errors.finish()
}

fn validate_governance_policy(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    validate_runtime_governance_settings(&policy.governance, path)?;
    if policy.governance.mode == RuntimeGovernanceMode::BankEnforce {
        validate_bank_deployment(policy, path)?;
    }
    Ok(())
}

pub fn validate_runtime_governance_settings(
    governance: &crate::types::RuntimePolicyGovernanceSettings,
    path: &Path,
) -> Result<()> {
    if governance.config_version != 1 {
        bail!("governance.config_version in {} must be 1", path.display());
    }
    validate_governance_authority_tenants(governance, path)?;
    validate_governance_inspection_patterns(governance, path)?;
    validate_governance_policy_rules(governance, path)?;
    validate_governance_session(governance, path)?;
    let enforcing = matches!(
        governance.mode,
        RuntimeGovernanceMode::EnterpriseEnforce | RuntimeGovernanceMode::BankEnforce
    );
    if enforcing
        && [
            governance.inspection,
            governance.classification,
            governance.policy,
            governance.routing,
        ]
        .into_iter()
        .any(|mode| mode != RuntimeGovernanceRolloutMode::Enforce)
    {
        bail!(
            "enforcing governance mode requires inspection, classification, policy, and routing enforcement in {}",
            path.display()
        );
    }
    if enforcing && !governance.mandatory_audit {
        bail!(
            "enforcing governance mode requires mandatory audit in {}",
            path.display()
        );
    }
    if governance.mode == RuntimeGovernanceMode::BankEnforce {
        if governance.anonymous_data_plane {
            bail!(
                "bank governance mode forbids anonymous data-plane access in {}",
                path.display()
            );
        }
        if governance.raw_secret_sources {
            bail!(
                "bank governance mode requires secret references in {}",
                path.display()
            );
        }
    }
    if enforcing {
        if governance.policy_revision.is_none()
            || governance
                .policy_valid_until_unix_ms
                .is_none_or(|value| value == 0)
            || governance
                .classification_revision
                .as_deref()
                .is_none_or(|value| !governance_token_is_valid(value))
            || governance
                .classification_checksum
                .as_deref()
                .is_none_or(|value| !governance_token_is_valid(value))
            || governance
                .provider_registry_revision
                .is_none_or(|value| value == 0)
            || governance
                .routing_score_revision
                .is_none_or(|value| value == 0)
        {
            bail!(
                "enforcing governance mode requires valid immutable snapshot revisions in {}",
                path.display()
            );
        }
        let Some(provider) = governance.provider.as_ref() else {
            bail!(
                "enforcing governance mode requires an approved provider registry entry in {}",
                path.display()
            );
        };
        if provider.descriptor_revision == 0
            || provider.regions.is_empty()
            || provider.regions.len() > 16
            || provider
                .regions
                .iter()
                .any(|region| !governance_token_is_valid(region))
        {
            bail!(
                "governance provider registry entry is invalid in {}",
                path.display()
            );
        }
        if governance.mode == RuntimeGovernanceMode::BankEnforce
            && (provider.trust_tier < crate::types::RuntimeGovernanceProviderTrustTier::Enterprise
                || provider.training_use
                || provider.retention_seconds != 0)
        {
            bail!(
                "bank governance mode requires an approved no-retention provider in {}",
                path.display()
            );
        }
        if governance.classification_unknown != RuntimeGovernanceUnknownClassificationBehavior::Deny
            || governance.policy_failure_mode != RuntimeGovernancePolicyFailureMode::Closed
            || governance.active_policy_revision.is_none()
            || governance.active_policy_revision != governance.policy_revision
        {
            bail!(
                "enforcing governance mode requires deny-on-unknown classification, fail-closed policy, and matching active policy revision in {}",
                path.display()
            );
        }
        if governance.session.absolute_timeout_seconds.is_none()
            || governance.session.idle_timeout_seconds.is_none()
            || governance.session.max_concurrent.is_none()
        {
            bail!(
                "enforcing governance mode requires explicit bounded session controls in {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn validate_governance_authority_tenants(
    governance: &crate::types::RuntimePolicyGovernanceSettings,
    path: &Path,
) -> Result<()> {
    use std::collections::BTreeSet;

    if governance.authority_tenants.len() > MAX_GOVERNANCE_AUTHORITY_TENANTS {
        bail!(
            "governance authority tenant count exceeds {} in {}",
            MAX_GOVERNANCE_AUTHORITY_TENANTS,
            path.display()
        );
    }
    let mut unique = BTreeSet::new();
    if governance
        .authority_tenants
        .iter()
        .any(|tenant_id| !unique.insert(*tenant_id))
    {
        bail!(
            "governance authority tenants contain duplicates in {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_governance_policy_rules(
    governance: &crate::types::RuntimePolicyGovernanceSettings,
    path: &Path,
) -> Result<()> {
    use crate::types::{RuntimeGovernancePolicyEffect, RuntimeGovernancePolicyObligation};
    use prodex_domain::{
        CanonicalRoute, GovernancePolicyRuleId, MAX_GOVERNANCE_POLICY_RULES,
        MAX_POLICY_OBLIGATIONS, PolicyReasonCode, PolicySelector,
    };
    use std::collections::BTreeSet;

    if governance.policy_rules.len() > MAX_GOVERNANCE_POLICY_RULES {
        bail!(
            "governance policy rule count exceeds {} in {}",
            MAX_GOVERNANCE_POLICY_RULES,
            path.display()
        );
    }
    let mut identities = BTreeSet::new();
    for rule in &governance.policy_rules {
        if GovernancePolicyRuleId::new(&rule.id).is_err()
            || PolicyReasonCode::new(&rule.reason_code).is_err()
            || !identities.insert(rule.id.as_str())
            || rule.id.starts_with("builtin.")
            || rule
                .condition
                .route
                .as_deref()
                .is_some_and(|route| CanonicalRoute::new(route).is_err())
        {
            bail!(
                "governance policy rule identity or condition is invalid in {}",
                path.display()
            );
        }
        if rule
            .condition
            .channel
            .is_some_and(|channel| channel != crate::types::RuntimeGovernancePolicyChannel::Api)
        {
            bail!(
                "gateway governance policy channel selector must be api in {}",
                path.display()
            );
        }
        for selector in [
            rule.condition.team_id.as_deref(),
            rule.condition.project_id.as_deref(),
            rule.condition.user_id.as_deref(),
            rule.condition.requested_model.as_deref(),
            rule.condition.requested_tool.as_deref(),
            rule.condition.break_glass_scope.as_deref(),
        ] {
            if selector.is_some_and(|selector| PolicySelector::new(selector).is_err()) {
                bail!(
                    "governance policy attribute selector is invalid in {}",
                    path.display()
                );
            }
        }
        if rule.obligations.len() > MAX_POLICY_OBLIGATIONS {
            bail!(
                "governance policy obligation count exceeds {} in {}",
                MAX_POLICY_OBLIGATIONS,
                path.display()
            );
        }
        if rule.effect == RuntimeGovernancePolicyEffect::Deny && !rule.obligations.is_empty() {
            bail!(
                "governance deny policy rules cannot carry obligations in {}",
                path.display()
            );
        }
        for obligation in &rule.obligations {
            let selector = match obligation {
                RuntimeGovernancePolicyObligation::AllowProvider { selector }
                | RuntimeGovernancePolicyObligation::DenyProvider { selector }
                | RuntimeGovernancePolicyObligation::RequireRegion { selector }
                | RuntimeGovernancePolicyObligation::AllowTool { selector }
                | RuntimeGovernancePolicyObligation::AllowModel { selector } => Some(selector),
                _ => None,
            };
            if selector.is_some_and(|selector| PolicySelector::new(selector).is_err()) {
                bail!(
                    "governance policy obligation selector is invalid in {}",
                    path.display()
                );
            }
            if matches!(
                obligation,
                RuntimeGovernancePolicyObligation::MaxInputTokens { value: 0 }
                    | RuntimeGovernancePolicyObligation::MaxOutputTokens { value: 0 }
                    | RuntimeGovernancePolicyObligation::MaxContextTokens { value: 0 }
                    | RuntimeGovernancePolicyObligation::SessionIdleTimeoutSeconds { value: 0 }
                    | RuntimeGovernancePolicyObligation::SessionAbsoluteTimeoutSeconds { value: 0 }
            ) {
                bail!(
                    "governance policy obligation bound must be non-zero in {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn validate_governance_session(
    governance: &crate::types::RuntimePolicyGovernanceSettings,
    path: &Path,
) -> Result<()> {
    let session = &governance.session;
    if session.absolute_timeout_seconds.is_some_and(|value| {
        !(MIN_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS
            ..=MAX_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS)
            .contains(&value)
    }) {
        bail!(
            "governance.session.absolute_timeout_seconds in {} is outside the safe range",
            path.display()
        );
    }
    if session.idle_timeout_seconds.is_some_and(|value| {
        !(MIN_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS..=MAX_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS)
            .contains(&value)
    }) {
        bail!(
            "governance.session.idle_timeout_seconds in {} is outside the safe range",
            path.display()
        );
    }
    if session
        .max_concurrent
        .is_some_and(|value| value == 0 || value > MAX_GOVERNANCE_SESSION_CONCURRENT)
    {
        bail!(
            "governance.session.max_concurrent in {} is outside the safe range",
            path.display()
        );
    }
    if let (Some(idle), Some(absolute)) = (
        session.idle_timeout_seconds,
        session.absolute_timeout_seconds,
    ) && idle > absolute
    {
        bail!(
            "governance.session.idle_timeout_seconds in {} cannot exceed absolute_timeout_seconds",
            path.display()
        );
    }
    Ok(())
}

fn validate_bank_deployment(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    let governance = &policy.governance;
    if governance.classification_default != RuntimeGovernanceDataClassification::Restricted {
        bail!(
            "bank governance mode requires restricted default classification in {}",
            path.display()
        );
    }
    if !policy.secrets.production {
        bail!(
            "bank governance mode requires production projected secret references in {}",
            path.display()
        );
    }
    if policy
        .gateway
        .listen_addr
        .as_deref()
        .is_none_or(|value| !bank_bind_address_is_private(value))
    {
        bail!(
            "bank governance mode requires a private gateway listen address in {}",
            path.display()
        );
    }
    if policy.gateway.expected_host.is_none() {
        bail!(
            "bank governance mode requires an exact gateway expected host in {}",
            path.display()
        );
    }
    if policy.gateway.restricted_egress != Some(true) {
        bail!(
            "bank governance mode requires restricted egress in {}",
            path.display()
        );
    }
    if policy
        .gateway
        .replica_count
        .is_none_or(|replicas| replicas < 2)
        || policy.gateway.require_multi_replica_accounting_checks != Some(true)
    {
        bail!(
            "bank governance mode requires a highly available gateway with multi-replica accounting checks in {}",
            path.display()
        );
    }
    if policy.gateway.state.backend.as_deref() != Some("postgres")
        || policy.gateway.state.postgres_url_ref.is_none()
        || policy.gateway.state.redis_url_ref.is_none()
    {
        bail!(
            "bank governance mode requires shared PostgreSQL state and Redis coordination SecretRefs in {}",
            path.display()
        );
    }
    let sso = &policy.gateway.sso;
    if sso.remote_human != Some(true)
        || sso.oidc_issuer.is_none()
        || sso.oidc_audience.is_none()
        || sso.required_scope.as_deref() != Some("control_plane")
        || sso.authentication_strength.as_deref() != Some("phishing_resistant")
    {
        bail!(
            "bank governance mode requires exact remote human OIDC issuer, audience, control_plane scope, and phishing_resistant authentication in {}",
            path.display()
        );
    }
    if sso.browser_flow == Some(true) {
        bail!(
            "bank governance mode cannot enable unsupported browser OIDC flow in {}",
            path.display()
        );
    }
    let observability = &policy.gateway.observability;
    if !observability.sinks.iter().any(|sink| sink == "siem")
        || observability.siem_endpoint.is_none()
        || observability.siem_bearer_token_ref.is_none()
        || observability.siem_mtls_identity_ref.is_none()
        || observability.siem_signing_key_ref.is_none()
        || observability.siem_max_batch_events.is_none()
        || observability.siem_max_batch_bytes.is_none()
        || observability.siem_max_attempts.is_none()
        || observability.siem_retry_base_ms.is_none()
        || observability.siem_retry_max_ms.is_none()
        || observability.siem_max_lag_ms.is_none()
    {
        bail!(
            "bank governance mode requires a bounded SecretRef/mTLS/signing SIEM audit worker in {}",
            path.display()
        );
    }
    Ok(())
}

fn bank_bind_address_is_private(value: &str) -> bool {
    let Ok(address) = value.parse::<std::net::SocketAddr>() else {
        return false;
    };
    match address.ip() {
        std::net::IpAddr::V4(ip) => ip.is_loopback() || ip.is_private(),
        std::net::IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local(),
    }
}

fn validate_governance_inspection_patterns(
    governance: &crate::types::RuntimePolicyGovernanceSettings,
    path: &Path,
) -> Result<()> {
    use std::collections::{BTreeMap, BTreeSet};

    if governance.inspection_patterns.len() > MAX_GOVERNANCE_INSPECTION_PATTERNS {
        bail!(
            "governance inspection pattern count exceeds {} in {}",
            MAX_GOVERNANCE_INSPECTION_PATTERNS,
            path.display()
        );
    }
    let mut per_tenant = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for entry in &governance.inspection_patterns {
        let count = per_tenant.entry(entry.tenant_id).or_insert(0usize);
        *count += 1;
        if *count > MAX_GOVERNANCE_INSPECTION_PATTERNS_PER_TENANT {
            bail!(
                "governance inspection pattern count exceeds per-tenant limit in {}",
                path.display()
            );
        }
        if entry.id.is_empty()
            || entry.id.len() > MAX_GOVERNANCE_INSPECTION_PATTERN_ID_BYTES
            || !entry
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            bail!(
                "governance inspection pattern id is invalid in {}",
                path.display()
            );
        }
        if !identities.insert((entry.tenant_id, entry.id.as_str())) {
            bail!(
                "governance inspection pattern id is duplicated for a tenant in {}",
                path.display()
            );
        }
        let wildcard_count = entry.pattern.bytes().filter(|byte| *byte == b'*').count();
        if entry.pattern.is_empty()
            || entry.pattern.len() > MAX_GOVERNANCE_INSPECTION_PATTERN_BYTES
            || wildcard_count > MAX_GOVERNANCE_INSPECTION_PATTERN_WILDCARDS
            || entry.pattern.chars().any(char::is_control)
            || entry.pattern.starts_with('*')
            || entry.pattern.ends_with('*')
            || entry.pattern.split('*').any(str::is_empty)
        {
            bail!(
                "governance inspection pattern must be a bounded literal or interior-star glob in {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn governance_token_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
}

fn validate_policy_version(policy: &RuntimePolicyFile, path: &Path) -> Result<()> {
    if policy.version != PRODEX_POLICY_VERSION {
        bail!(
            "unsupported prodex policy version {} in {}; expected {}",
            policy.version,
            path.display(),
            PRODEX_POLICY_VERSION
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/src/validate.rs"]
mod tests;
