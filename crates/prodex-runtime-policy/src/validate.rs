use crate::types::{
    PRODEX_POLICY_VERSION, RuntimeGovernanceMode, RuntimeGovernanceRolloutMode, RuntimePolicyFile,
};
use crate::validate_secrets::validate_secret_policy;
use anyhow::{Result, bail};
use std::path::Path;

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
    let governance = &policy.governance;
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
    if governance.mode == RuntimeGovernanceMode::BankEnforce {
        if !governance.mandatory_audit {
            bail!(
                "bank governance mode requires mandatory audit in {}",
                path.display()
            );
        }
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
