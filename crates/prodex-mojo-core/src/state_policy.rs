use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Clone, Copy)]
enum StatePolicyMode {
    ProviderCapabilities = 0,
    BindingMerge = 1,
    ActiveProfile = 2,
    LastRunKeep = 3,
    BindingKeep = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderCapabilitiesPlan {
    pub route_policy: i64,
    pub quota_shape: i64,
    pub uses_openai_client_format: bool,
    pub supports_runtime_rotation: bool,
    pub provider_priority: usize,
    pub supports_remote_compact_affinity: bool,
    pub supports_websocket_reuse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingMergeChoice {
    Conflict,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingMergePlan {
    pub choice: BindingMergeChoice,
    pub bound_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveProfileChoice {
    None,
    Existing,
    Incoming,
}

unsafe extern "C" {
    fn prodex_state_policy_v1(
        abi_version: i64,
        mode: i64,
        input0: i64,
        input1: i64,
        input2: i64,
        input3: i64,
        input4: i64,
        input5: i64,
        input6: i64,
        input7: i64,
        output_address: u64,
    ) -> i64;
}

fn call(mode: StatePolicyMode, input: [i64; 8]) -> Result<[i64; 8], MojoError> {
    let mut output = [0_i64; 8];
    let status = unsafe {
        prodex_state_policy_v1(
            ABI_VERSION,
            mode as i64,
            input[0],
            input[1],
            input[2],
            input[3],
            input[4],
            input[5],
            input[6],
            input[7],
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn output_bool(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn provider_capabilities(provider_kind: u8) -> Result<ProviderCapabilitiesPlan, MojoError> {
    let output = call(
        StatePolicyMode::ProviderCapabilities,
        [i64::from(provider_kind), 0, 0, 0, 0, 0, 0, 0],
    )?;
    if !(0..=2).contains(&output[0]) || !(0..=3).contains(&output[1]) {
        return Err(MojoError::InvalidOutput);
    }
    let provider_priority = usize::try_from(output[4]).map_err(|_| MojoError::InvalidOutput)?;
    Ok(ProviderCapabilitiesPlan {
        route_policy: output[0],
        quota_shape: output[1],
        uses_openai_client_format: output_bool(output[2])?,
        supports_runtime_rotation: output_bool(output[3])?,
        provider_priority,
        supports_remote_compact_affinity: output_bool(output[5])?,
        supports_websocket_reuse: output_bool(output[6])?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn binding_merge_plan(
    left_conflict: bool,
    right_conflict: bool,
    profile_names_equal: bool,
    left_identity_present: bool,
    right_identity_present: bool,
    identities_conflict: bool,
    left_bound_at: i64,
    right_bound_at: i64,
) -> Result<BindingMergePlan, MojoError> {
    let output = call(
        StatePolicyMode::BindingMerge,
        [
            i64::from(left_conflict),
            i64::from(right_conflict),
            i64::from(profile_names_equal),
            i64::from(left_identity_present),
            i64::from(right_identity_present),
            i64::from(identities_conflict),
            left_bound_at,
            right_bound_at,
        ],
    )?;
    let choice = match output[0] {
        0 => BindingMergeChoice::Conflict,
        1 => BindingMergeChoice::Left,
        2 => BindingMergeChoice::Right,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(BindingMergePlan {
        choice,
        bound_at: output[1],
    })
}

pub fn active_profile_choice(
    existing_present: bool,
    incoming_present: bool,
    names_equal: bool,
    existing_selected_at: Option<i64>,
    incoming_selected_at: Option<i64>,
) -> Result<ActiveProfileChoice, MojoError> {
    let output = call(
        StatePolicyMode::ActiveProfile,
        [
            i64::from(existing_present),
            i64::from(incoming_present),
            i64::from(names_equal),
            i64::from(existing_selected_at.is_some()),
            i64::from(incoming_selected_at.is_some()),
            existing_selected_at.unwrap_or_default(),
            incoming_selected_at.unwrap_or_default(),
            0,
        ],
    )?;
    match output[0] {
        0 => Ok(ActiveProfileChoice::None),
        1 => Ok(ActiveProfileChoice::Existing),
        2 => Ok(ActiveProfileChoice::Incoming),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn last_run_selection_keep(
    profile_exists: bool,
    timestamp: i64,
    now: i64,
    retention_seconds: i64,
) -> Result<bool, MojoError> {
    let output = call(
        StatePolicyMode::LastRunKeep,
        [
            i64::from(profile_exists),
            timestamp,
            now,
            retention_seconds,
            0,
            0,
            0,
            0,
        ],
    )?;
    output_bool(output[0])
}

pub fn binding_keep(
    conflict: bool,
    identity_present: bool,
    profile_exists: bool,
    bound_at: i64,
    now: i64,
    retention_seconds: i64,
    apply_retention: bool,
) -> Result<bool, MojoError> {
    let output = call(
        StatePolicyMode::BindingKeep,
        [
            i64::from(conflict),
            i64::from(identity_present),
            i64::from(profile_exists),
            bound_at,
            now,
            retention_seconds,
            i64::from(apply_retention),
            0,
        ],
    )?;
    output_bool(output[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_policy_kernel_smoke() {
        let openai = provider_capabilities(0).unwrap();
        assert_eq!(openai.route_policy, 0);
        assert_eq!(openai.quota_shape, 0);
        assert!(openai.supports_runtime_rotation);
        assert_eq!(openai.provider_priority, 0);

        let gemini = provider_capabilities(1).unwrap();
        assert_eq!(gemini.route_policy, 1);
        assert_eq!(gemini.quota_shape, 1);
        assert!(!gemini.supports_runtime_rotation);

        assert_eq!(
            binding_merge_plan(false, false, true, false, true, false, 10, 20).unwrap(),
            BindingMergePlan {
                choice: BindingMergeChoice::Right,
                bound_at: 20,
            }
        );
        assert_eq!(
            active_profile_choice(true, true, false, Some(20), Some(10)).unwrap(),
            ActiveProfileChoice::Existing
        );
        assert!(last_run_selection_keep(true, 10, 20, 10).unwrap());
        assert!(!last_run_selection_keep(true, 9, 20, 10).unwrap());
        assert!(binding_keep(true, false, false, i64::MIN, i64::MAX, 0, true).unwrap());
    }
}
