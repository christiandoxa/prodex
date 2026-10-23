//! Checked Mojo classification of inherited child-process environment names.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_KEYS: usize = 8192;
const MAX_KEY_BYTES: usize = 65_536;

unsafe extern "C" {
    fn prodex_launch_child_env_flags_v1(
        abi_version: i64,
        keys_address: u64,
        count: i64,
        flags_address: u64,
        flags_capacity: i64,
        hardening_bypass_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildEnvironmentRemovalPlan {
    pub sandbox_key_indices: Vec<usize>,
    pub dyld_key_indices: Vec<usize>,
    pub allow_unsafe_child_env: bool,
}

/// Compiled Mojo decides prefix matches and the exact opt-out key. Invalid
/// ABI input or output returns an error; callers do not retry in Rust.
pub fn child_environment_removal_plan(
    keys: &[Option<&str>],
) -> Result<ChildEnvironmentRemovalPlan, MojoError> {
    if keys.len() > MAX_KEYS || keys.iter().flatten().any(|key| key.len() > MAX_KEY_BYTES) {
        return Err(MojoError::InvalidInput);
    }
    let views = keys
        .iter()
        .map(|key| match key {
            Some(value) => [
                value.as_ptr() as usize as u64,
                u64::try_from(value.len()).expect("child env key length is bounded"),
                1_u64,
            ],
            None => [0_u64, 0_u64, 0_u64],
        })
        .collect::<Vec<_>>();
    let mut tags = vec![-1_i64; keys.len()];
    let mut allow_unsafe = -1_i64;
    let status = unsafe {
        prodex_launch_child_env_flags_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(keys.len()).map_err(|_| MojoError::InvalidInput)?,
            tags.as_mut_ptr() as usize as u64,
            i64::try_from(tags.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut allow_unsafe as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let allow_unsafe_child_env = match allow_unsafe {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut sandbox_key_indices = Vec::new();
    let mut dyld_key_indices = Vec::new();
    let mut actual_bypass = false;
    for (index, tag) in tags.into_iter().enumerate() {
        if !(0..=7).contains(&tag) {
            return Err(MojoError::InvalidOutput);
        }
        if tag & 1 != 0 {
            sandbox_key_indices.push(index);
        }
        if tag & 2 != 0 {
            dyld_key_indices.push(index);
        }
        actual_bypass |= tag & 4 != 0;
    }
    if actual_bypass != allow_unsafe_child_env {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ChildEnvironmentRemovalPlan {
        sandbox_key_indices,
        dyld_key_indices,
        allow_unsafe_child_env,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiled_child_env_policy_recognizes_exact_names_and_opaque_keys() {
        assert_eq!(
            child_environment_removal_plan(&[
                Some("CODEX_SANDBOX"),
                Some("CODEX_SANDBOX_NETWORK_DISABLED"),
                Some("DYLD_INSERT_LIBRARIES"),
                Some("dyld_custom"),
                None,
                Some("PRODEX_ALLOW_UNSAFE_CHILD_ENV"),
                Some("CODEX_SANDBOX_SUFFIX")
            ]),
            Ok(ChildEnvironmentRemovalPlan {
                sandbox_key_indices: vec![0, 1, 6],
                dyld_key_indices: vec![2],
                allow_unsafe_child_env: true,
            })
        );
        assert_eq!(
            child_environment_removal_plan(&[]),
            Ok(ChildEnvironmentRemovalPlan {
                sandbox_key_indices: vec![],
                dyld_key_indices: vec![],
                allow_unsafe_child_env: false,
            })
        );
    }

    #[test]
    fn compiled_child_env_policy_rejects_unbounded_names() {
        let invalid = "X".repeat(MAX_KEY_BYTES + 1);
        assert_eq!(
            child_environment_removal_plan(&[Some(&invalid)]),
            Err(MojoError::InvalidInput)
        );
    }
}
