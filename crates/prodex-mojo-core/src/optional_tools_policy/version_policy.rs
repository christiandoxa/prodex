//! Checked borrowed inputs for compiled optional-tool version and daemon policy.
use super::{ABI_VERSION, OptionalToolStringView};
use crate::MojoError;

const MAX_TEXT_BYTES: usize = 1_048_576;

#[repr(i64)]
enum VersionPolicy {
    ExactMinimum,
    CodebaseVersion,
    CodebaseDaemon,
    StableMinimum,
}

/// Exact stable version validation before installation identity checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalToolStableVersion {
    Malformed,
    Incompatible,
    Supported,
}

unsafe extern "C" {
    fn prodex_optional_tool_version_policy_v1(
        abi_version: i64,
        operation: i64,
        success: i64,
        input_address: u64,
        input_length: i64,
        minimum_address: u64,
        minimum_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_optional_tool_newest_stable_version_v1(
        abi_version: i64,
        names_address: u64,
        count: i64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn check(
    policy: VersionPolicy,
    text: &str,
    minimum: &str,
    success: bool,
) -> Result<i64, MojoError> {
    if text.len() > MAX_TEXT_BYTES || minimum.len() > MAX_TEXT_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = -1_i64;
    status(unsafe {
        prodex_optional_tool_version_policy_v1(
            ABI_VERSION,
            policy as i64,
            i64::from(success),
            text.as_ptr() as u64,
            text.len() as i64,
            minimum.as_ptr() as u64,
            minimum.len() as i64,
            &mut output as *mut i64 as u64,
        )
    })?;
    Ok(output)
}

fn bool_check(
    policy: VersionPolicy,
    text: &str,
    minimum: &str,
    success: bool,
) -> Result<bool, MojoError> {
    match check(policy, text, minimum, success)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Compare exact SemVer, including prereleases and build metadata total ordering.
/// An invalid minimum is an input error; a malformed candidate is unsupported.
pub fn optional_tool_version_at_least(version: &str, minimum: &str) -> Result<bool, MojoError> {
    bool_check(VersionPolicy::ExactMinimum, version, minimum, false)
}

/// Enforce the exact Codebase Memory version line, including its `dev` exception.
pub fn codebase_memory_version_supported(line: &str, minimum: &str) -> Result<bool, MojoError> {
    bool_check(VersionPolicy::CodebaseVersion, line, minimum, false)
}

/// Accept an active daemon only after success, or an initially stopped daemon.
pub fn codebase_memory_daemon_status_supported(
    success: bool,
    line: &str,
) -> Result<bool, MojoError> {
    bool_check(VersionPolicy::CodebaseDaemon, line, "", success)
}

/// Distinguish invalid SemVer syntax from a prerelease or an obsolete version.
pub fn optional_tool_stable_version(
    version: &str,
    minimum: &str,
) -> Result<OptionalToolStableVersion, MojoError> {
    match check(VersionPolicy::StableMinimum, version, minimum, false)? {
        0 => Ok(OptionalToolStableVersion::Malformed),
        1 => Ok(OptionalToolStableVersion::Incompatible),
        2 => Ok(OptionalToolStableVersion::Supported),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Select the first newest stable exact-SemVer name from filesystem observations.
/// Rust retains the associated paths and all filesystem trust checks.
pub fn optional_tool_newest_stable_version(names: &[&str]) -> Result<Option<usize>, MojoError> {
    if names.len() > 1_048_576 || names.iter().any(|name| name.len() > MAX_TEXT_BYTES) {
        return Err(MojoError::InvalidInput);
    }
    let views = names
        .iter()
        .map(|name| OptionalToolStringView {
            ptr: name.as_ptr() as u64,
            len: name.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut output = -2_i64;
    status(unsafe {
        prodex_optional_tool_newest_stable_version_v1(
            ABI_VERSION,
            views.as_ptr() as u64,
            views.len() as i64,
            &mut output as *mut i64 as u64,
        )
    })?;
    if output == -1 {
        return Ok(None);
    }
    let index = usize::try_from(output).map_err(|_| MojoError::InvalidOutput)?;
    if index >= names.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(index))
}

#[cfg(test)]
#[path = "version_policy_tests.rs"]
mod tests;
