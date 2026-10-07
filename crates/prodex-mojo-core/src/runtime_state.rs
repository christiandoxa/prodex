use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MODE_MUTATION_PLAN: i64 = 0;
const MODE_QUEUE_PRESSURE: i64 = 1;
const MODE_QUEUE_ENQUEUE: i64 = 2;
const MODE_ENQUEUE_BACKLOG: i64 = 3;
const MODE_QUEUE_THRESHOLD: i64 = 4;
const MODE_ADMISSION_PLAN: i64 = 5;
const MODE_PROFILE_INFLIGHT_ACQUIRE: i64 = 6;
const MODE_PROFILE_INFLIGHT_RELEASE: i64 = 7;
const MODE_LANE_LIMIT: i64 = 8;

const RUNTIME_PROXY_ADMISSION_ABI_VERSION: i64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeStateMutationPolicy {
    pub state_section: u8,
    pub continuations: bool,
    pub profile_scores: bool,
    pub usage_snapshots: bool,
    pub backoffs: bool,
    pub requires_continuation_journal: bool,
    pub hot_continuation_state: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeBackgroundQueuePlan {
    pub backlog: usize,
    pub pressure_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeAdmissionPlan {
    Allow {
        next_active: usize,
        next_lane_active: usize,
        bypassed_lane_limit: bool,
    },
    GlobalLimit {
        active: usize,
        limit: usize,
    },
    LaneLimit {
        active: usize,
        limit: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeProfileInflightAcquirePlan {
    pub accepted: bool,
    pub next: usize,
    pub weight: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeProfileInflightReleasePlan {
    pub remaining: usize,
    pub count_before: usize,
    pub underflow: bool,
    pub weight: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeProxyAdmissionPolicy {
    pub background_queue_pressure_affects_route: bool,
    pub pressure_mode: bool,
    pub lane_limit_marks_global_overload: bool,
    pub shed_fresh_compact_request: bool,
}

unsafe extern "C" {
    fn prodex_runtime_state_background_policy_v1(
        abi_version: i64,
        mode: i64,
        mutation_kind: i64,
        queue_kind: i64,
        state_save_backlog: u64,
        continuation_journal_backlog: u64,
        probe_refresh_backlog: u64,
        state_save_threshold: u64,
        continuation_journal_threshold: u64,
        probe_refresh_threshold: u64,
        pending_len_after_enqueue: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_state_mutation_reason_v1(
        abi_version: i64,
        mutation_kind: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_proxy_admission_policy_v1(
        abi_version: i64,
        route_kind: i64,
        local_overload_pressure: i64,
        background_queue_pressure: i64,
        session_profile_present: i64,
        output_address: u64,
    ) -> i64;
}

fn bool_output(value: u64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn usize_output(value: u64) -> Result<usize, MojoError> {
    usize::try_from(value).map_err(|_| MojoError::InvalidOutput)
}

fn call(
    mode: i64,
    mutation_kind: i64,
    queue_kind: i64,
    backlogs: [usize; 3],
    thresholds: [usize; 3],
    pending_len_after_enqueue: usize,
) -> Result<[u64; 8], MojoError> {
    let mut output = [0_u64; 8];
    let status = unsafe {
        prodex_runtime_state_background_policy_v1(
            ABI_VERSION,
            mode,
            mutation_kind,
            queue_kind,
            u64::try_from(backlogs[0]).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(backlogs[1]).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(backlogs[2]).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(thresholds[0]).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(thresholds[1]).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(thresholds[2]).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(pending_len_after_enqueue).map_err(|_| MojoError::InvalidInput)?,
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

pub fn mutation_reason(kind: u8, value: Option<&str>) -> Result<String, MojoError> {
    let value = value.unwrap_or_default();
    let capacity = value.len().checked_add(64).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_runtime_state_mutation_reason_v1(
            ABI_VERSION,
            i64::from(kind),
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

pub fn mutation_policy(kind: u8) -> Result<RuntimeStateMutationPolicy, MojoError> {
    let output = call(MODE_MUTATION_PLAN, i64::from(kind), 0, [0; 3], [0; 3], 0)?;
    let state_section = u8::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if state_section > 2 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RuntimeStateMutationPolicy {
        state_section,
        continuations: bool_output(output[1])?,
        profile_scores: bool_output(output[2])?,
        usage_snapshots: bool_output(output[3])?,
        backoffs: bool_output(output[4])?,
        requires_continuation_journal: bool_output(output[5])?,
        hot_continuation_state: bool_output(output[6])?,
    })
}

pub fn queue_pressure_active(
    backlogs: [usize; 3],
    thresholds: [usize; 3],
) -> Result<bool, MojoError> {
    bool_output(call(MODE_QUEUE_PRESSURE, 0, 0, backlogs, thresholds, 0)?[0])
}

pub fn enqueue_backlog(pending_len_after_enqueue: usize) -> Result<usize, MojoError> {
    usize_output(
        call(
            MODE_ENQUEUE_BACKLOG,
            0,
            0,
            [0; 3],
            [0; 3],
            pending_len_after_enqueue,
        )?[0],
    )
}

pub fn queue_threshold(queue_kind: u8, thresholds: [usize; 3]) -> Result<usize, MojoError> {
    usize_output(
        call(
            MODE_QUEUE_THRESHOLD,
            0,
            i64::from(queue_kind),
            [0; 3],
            thresholds,
            0,
        )?[0],
    )
}

pub fn queue_enqueue_plan(
    queue_kind: u8,
    pending_len_after_enqueue: usize,
    thresholds: [usize; 3],
) -> Result<RuntimeBackgroundQueuePlan, MojoError> {
    let output = call(
        MODE_QUEUE_ENQUEUE,
        0,
        i64::from(queue_kind),
        [0; 3],
        thresholds,
        pending_len_after_enqueue,
    )?;
    Ok(RuntimeBackgroundQueuePlan {
        backlog: usize_output(output[0])?,
        pressure_active: bool_output(output[1])?,
    })
}

pub fn admission_plan(
    active: usize,
    active_limit: usize,
    lane_active: usize,
    lane_limit: usize,
    bypass_lane_limit: bool,
) -> Result<RuntimeAdmissionPlan, MojoError> {
    let output = call(
        MODE_ADMISSION_PLAN,
        i64::from(bypass_lane_limit),
        0,
        [active, lane_active, 0],
        [active_limit, lane_limit, 0],
        0,
    )?;
    match output[0] {
        0 => Ok(RuntimeAdmissionPlan::Allow {
            next_active: usize_output(output[1])?,
            next_lane_active: usize_output(output[2])?,
            bypassed_lane_limit: bool_output(output[3])?,
        }),
        1 => Ok(RuntimeAdmissionPlan::GlobalLimit {
            active: usize_output(output[1])?,
            limit: usize_output(output[2])?,
        }),
        2 => Ok(RuntimeAdmissionPlan::LaneLimit {
            active: usize_output(output[1])?,
            limit: usize_output(output[2])?,
        }),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn profile_inflight_acquire_plan(
    current: usize,
    weight: usize,
    hard_limit: Option<usize>,
) -> Result<RuntimeProfileInflightAcquirePlan, MojoError> {
    let output = call(
        MODE_PROFILE_INFLIGHT_ACQUIRE,
        i64::from(hard_limit.is_some()),
        0,
        [current, weight, 0],
        [hard_limit.unwrap_or_default(), 0, 0],
        0,
    )?;
    Ok(RuntimeProfileInflightAcquirePlan {
        accepted: bool_output(output[0])?,
        next: usize_output(output[1])?,
        weight: usize_output(output[2])?,
    })
}

pub fn profile_inflight_release_plan(
    current: Option<usize>,
    weight: usize,
) -> Result<RuntimeProfileInflightReleasePlan, MojoError> {
    let output = call(
        MODE_PROFILE_INFLIGHT_RELEASE,
        i64::from(current.is_some()),
        0,
        [current.unwrap_or_default(), weight, 0],
        [0; 3],
        0,
    )?;
    Ok(RuntimeProfileInflightReleasePlan {
        remaining: usize_output(output[0])?,
        count_before: usize_output(output[1])?,
        underflow: bool_output(output[2])?,
        weight: usize_output(output[3])?,
    })
}

pub fn lane_limit(route_kind: u8, limits: [usize; 4]) -> Result<usize, MojoError> {
    usize_output(
        call(
            MODE_LANE_LIMIT,
            0,
            i64::from(route_kind),
            [limits[0], limits[1], limits[2]],
            [0; 3],
            limits[3],
        )?[0],
    )
}

pub fn runtime_proxy_admission_policy(
    route_kind: u8,
    local_overload_pressure: bool,
    background_queue_pressure: bool,
    session_profile_present: bool,
) -> Result<RuntimeProxyAdmissionPolicy, MojoError> {
    let mut output = [0_u64; 4];
    let status = unsafe {
        prodex_runtime_proxy_admission_policy_v1(
            RUNTIME_PROXY_ADMISSION_ABI_VERSION,
            i64::from(route_kind),
            i64::from(local_overload_pressure),
            i64::from(background_queue_pressure),
            i64::from(session_profile_present),
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(RuntimeProxyAdmissionPolicy {
            background_queue_pressure_affects_route: bool_output(output[0])?,
            pressure_mode: bool_output(output[1])?,
            lane_limit_marks_global_overload: bool_output(output[2])?,
            shed_fresh_compact_request: bool_output(output[3])?,
        }),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_state_background_kernel_smoke() {
        let policy = mutation_policy(9).unwrap();
        assert_eq!(policy.state_section, 1);
        assert!(policy.continuations);
        assert!(policy.requires_continuation_journal);
        assert!(policy.hot_continuation_state);

        assert_eq!(enqueue_backlog(0).unwrap(), 0);
        assert_eq!(enqueue_backlog(3).unwrap(), 2);
        assert!(queue_pressure_active([8, 0, 0], [8, 8, 16]).unwrap());
        assert_eq!(
            queue_enqueue_plan(2, 18, [8, 8, 16]).unwrap(),
            RuntimeBackgroundQueuePlan {
                backlog: 17,
                pressure_active: true,
            }
        );
        assert_eq!(
            admission_plan(1, 2, 2, 2, true).unwrap(),
            RuntimeAdmissionPlan::Allow {
                next_active: 2,
                next_lane_active: 3,
                bypassed_lane_limit: true,
            }
        );
        assert_eq!(
            admission_plan(2, 2, 0, 2, false).unwrap(),
            RuntimeAdmissionPlan::GlobalLimit {
                active: 2,
                limit: 2,
            }
        );
        assert_eq!(
            profile_inflight_acquire_plan(1, 1, Some(2)).unwrap(),
            RuntimeProfileInflightAcquirePlan {
                accepted: true,
                next: 2,
                weight: 1,
            }
        );
        assert_eq!(
            profile_inflight_release_plan(Some(1), 2).unwrap(),
            RuntimeProfileInflightReleasePlan {
                remaining: 0,
                count_before: 1,
                underflow: true,
                weight: 2,
            }
        );
        assert_eq!(lane_limit(3, [1, 2, 3, 4]).unwrap(), 4);
        assert_eq!(
            runtime_proxy_admission_policy(1, false, true, false).unwrap(),
            RuntimeProxyAdmissionPolicy {
                background_queue_pressure_affects_route: true,
                pressure_mode: true,
                lane_limit_marks_global_overload: false,
                shed_fresh_compact_request: true,
            }
        );
        assert!(
            !runtime_proxy_admission_policy(0, false, true, false)
                .unwrap()
                .pressure_mode
        );
    }
}
