use crate::types::{PRODEX_POLICY_VERSION, RuntimePolicyFile};
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
    gateway::collect_gateway_policy_errors(&mut errors, policy, path);
    errors.finish()
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
