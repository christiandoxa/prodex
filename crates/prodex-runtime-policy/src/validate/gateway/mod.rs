mod core;
mod identity;
mod routing;
mod telemetry;

use super::{RuntimePolicyValidationErrors, RuntimePolicyValidationSection};
use crate::types::RuntimePolicyFile;
use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn collect_gateway_policy_errors(
    errors: &mut RuntimePolicyValidationErrors,
    policy: &RuntimePolicyFile,
    path: &Path,
) {
    for (section, result) in [
        (
            RuntimePolicyValidationSection::Gateway,
            core::validate_gateway_core(policy, path),
        ),
        (
            RuntimePolicyValidationSection::GatewayState,
            core::validate_gateway_state(policy, path),
        ),
        (
            RuntimePolicyValidationSection::GatewayAdminTokens,
            identity::validate_gateway_admin_tokens(policy, path),
        ),
        (
            RuntimePolicyValidationSection::GatewaySso,
            identity::validate_gateway_sso(policy, path),
        ),
        (
            RuntimePolicyValidationSection::GatewayRouting,
            routing::validate_gateway_routing(policy, path),
        ),
        (
            RuntimePolicyValidationSection::GatewayVirtualKeys,
            identity::validate_gateway_virtual_keys(policy, path),
        ),
        (
            RuntimePolicyValidationSection::GatewayObservability,
            telemetry::validate_gateway_observability(policy, path),
        ),
        (
            RuntimePolicyValidationSection::GatewayGuardrails,
            telemetry::validate_gateway_guardrails(policy, path),
        ),
    ] {
        errors.capture(section, result);
    }
}

pub(super) fn validate_gateway_exact_identifier(
    value: &str,
    path: &Path,
    field: &str,
) -> Result<()> {
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        bail!(
            "{field} in {} must be non-empty without whitespace",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn validate_gateway_optional_scope(
    value: Option<&str>,
    path: &Path,
    field: &str,
    name: &str,
) -> Result<()> {
    if let Some(value) = value {
        validate_gateway_exact_identifier(value, path, &format!("{field}.{name}"))?;
    }
    Ok(())
}
