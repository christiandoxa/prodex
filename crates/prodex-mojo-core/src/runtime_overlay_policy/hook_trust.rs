//! Versioned compiled Codex hooks/list and config/batchWrite status policy.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_HOOKS: usize = 65_536;
const MAX_STATUS_BYTES: usize = 65_536;

#[repr(C)]
#[derive(Clone, Copy)]
struct StatusView {
    address: u64,
    length: u64,
}

unsafe extern "C" {
    fn prodex_runtime_overlay_hook_trust_policy_v1(
        abi_version: i64,
        operation: i64,
        statuses_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn call(operation: i64, statuses: &[&str]) -> Result<Vec<i64>, MojoError> {
    if statuses.len() > MAX_HOOKS || statuses.iter().any(|s| s.len() > MAX_STATUS_BYTES) {
        return Err(MojoError::InvalidInput);
    }
    let views = statuses
        .iter()
        .map(|status| StatusView {
            address: status.as_ptr() as usize as u64,
            length: status.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut output = vec![-1_i64; views.len()];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_overlay_hook_trust_policy_v1(
            ABI_VERSION,
            operation,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(count);
    Ok(output)
}

/// Input order preserved; Mojo selects exactly the hooks requiring trust.
pub fn untrusted_hook_indices(statuses: &[&str]) -> Result<Vec<usize>, MojoError> {
    let output = call(0, statuses)?;
    let mut prior = None;
    output
        .into_iter()
        .map(|raw| {
            let index = usize::try_from(raw).map_err(|_| MojoError::InvalidOutput)?;
            if index >= statuses.len() || prior.is_some_and(|prior| prior >= index) {
                return Err(MojoError::InvalidOutput);
            }
            prior = Some(index);
            Ok(index)
        })
        .collect()
}

/// Compiled success classification; invalid output never becomes success.
pub fn hook_config_write_succeeded(status: &str) -> Result<bool, MojoError> {
    match call(1, &[status])?.as_slice() {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_hook_trust_statuses_preserve_exact_case_and_input_order() {
        assert_eq!(
            untrusted_hook_indices(&[
                "trusted",
                "untrusted",
                "modified",
                "Untrusted",
                "modified",
                "",
                "unknown"
            ]),
            Ok(vec![1, 2, 4])
        );
        assert_eq!(untrusted_hook_indices(&[]), Ok(vec![]));
        assert_eq!(
            untrusted_hook_indices(&["modified", "untrusted"]),
            Ok(vec![0, 1])
        );
    }

    #[test]
    fn compiled_hook_config_write_success_is_exact() {
        assert_eq!(hook_config_write_succeeded("ok"), Ok(true));
        assert_eq!(hook_config_write_succeeded("okOverridden"), Ok(true));
        assert_eq!(hook_config_write_succeeded("Ok"), Ok(false));
        assert_eq!(hook_config_write_succeeded("okoverridden"), Ok(false));
        assert_eq!(hook_config_write_succeeded("error"), Ok(false));
        assert_eq!(hook_config_write_succeeded(""), Ok(false));
        assert_eq!(
            untrusted_hook_indices(&[&"a".repeat(MAX_STATUS_BYTES + 1)]),
            Err(MojoError::InvalidInput)
        );
    }
}
