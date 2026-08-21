use anyhow::{Result, bail};
use std::path::Path;

use crate::types::RuntimePolicyFile;

pub(super) fn validate_gateway_request_constraints(
    policy: &RuntimePolicyFile,
    path: &Path,
) -> Result<()> {
    let constraints = &policy.gateway.request_constraints;
    if let Some(value) = constraints.unknown_context.as_deref()
        && !matches!(value, "allow" | "safe_window" | "reject")
    {
        bail!(
            "gateway.request_constraints.unknown_context in {} must be allow, safe_window, or reject",
            path.display()
        );
    }
    validate_gateway_safe_window_numeric(constraints.safe_window_tokens, path)?;
    if let Some(value) = constraints.oversized_output.as_deref()
        && !matches!(value, "passthrough" | "reject" | "clamp_with_notice")
    {
        bail!(
            "gateway.request_constraints.oversized_output in {} must be passthrough, reject, or clamp_with_notice",
            path.display()
        );
    }
    Ok(())
}

fn validate_gateway_safe_window_numeric(value: Option<u64>, path: &Path) -> Result<()> {
    #[cfg(feature = "mojo")]
    {
        let Some(value) = value else {
            return Ok(());
        };
        let rule = prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_NON_ZERO,
            value,
            minimum: 0,
            maximum: u64::MAX,
            related_value: 0,
        };
        let failed = prodex_mojo_core::policy::validate_numeric_rules(&[rule]).map_err(|_| {
            anyhow::anyhow!("gateway request constraint numeric validation returned invalid output")
        })?;
        if !failed.is_empty() {
            bail!(
                "gateway.request_constraints.safe_window_tokens in {} must be greater than 0",
                path.display()
            );
        }
        Ok(())
    }
    #[cfg(not(feature = "mojo"))]
    {
        let Some(value) = value else {
            return Ok(());
        };
        if value == 0 {
            bail!(
                "gateway.request_constraints.safe_window_tokens in {} must be greater than 0",
                path.display()
            );
        }
        Ok(())
    }
}
