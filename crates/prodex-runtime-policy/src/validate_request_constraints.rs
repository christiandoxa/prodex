use anyhow::{Result, bail};
use std::path::Path;

use crate::types::RuntimePolicyFile;

pub(super) fn validate_gateway_request_constraints(
    policy: &RuntimePolicyFile,
    path: &Path,
    safe_window_failed: bool,
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
    if safe_window_failed {
        bail!(
            "gateway.request_constraints.safe_window_tokens in {} must be greater than 0",
            path.display()
        );
    }
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
