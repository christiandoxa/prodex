use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerProcessIdentityPlan {
    Absent,
    Proven,
    OwnershipChanged,
    OwnershipUnproven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerTerminationSignalAction {
    Skip,
    Signal,
    Refuse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerLeaseLifecycleOperation {
    Cleanup,
    Acquire,
    Renew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerLeaseLifecycleAction {
    Ignore,
    Keep,
    Remove,
    Acquire,
    Renew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerRegistryProcessAction {
    Keep,
    Remove,
    DiscardStale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerReadinessDecision {
    Wait,
    Ready,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerIdleDecision {
    Wait,
    Reset,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerTerminationOutcomePlan {
    Cleanup,
    Failure,
    DiscardStale,
}

unsafe extern "C" {
    fn prodex_runtime_broker_process_identity_plan_v1(
        abi_version: i64,
        process_absence_proven: i64,
        expected_birth_present: i64,
        birth_present: i64,
        birth_matches: i64,
        path_check_enabled: i64,
        path_present: i64,
        path_matches: i64,
        recheck_enabled: i64,
        recheck_present: i64,
        recheck_matches: i64,
    ) -> i64;

    fn prodex_runtime_broker_termination_signal_plan_v1(
        abi_version: i64,
        identity_outcome: i64,
    ) -> i64;

    fn prodex_runtime_broker_lease_lifecycle_plan_v1(
        abi_version: i64,
        operation: i64,
        pid_valid: i64,
        process_absence_proven: i64,
        process_alive: i64,
        active_requests: u64,
        expired: i64,
    ) -> i64;

    fn prodex_runtime_broker_registry_process_plan_v1(
        abi_version: i64,
        identity_outcome: i64,
        active_requests: u64,
        live_leases: u64,
    ) -> i64;

    fn prodex_runtime_broker_readiness_plan_v1(
        abi_version: i64,
        registry_present: i64,
        instance_matches: i64,
        health_present: i64,
        health_matches: i64,
        elapsed_ms: u64,
        timeout_ms: u64,
    ) -> i64;

    fn prodex_runtime_broker_idle_plan_v1(
        abi_version: i64,
        startup_grace_elapsed: i64,
        active_requests: u64,
        live_leases: u64,
        idle_elapsed_seconds: i64,
        idle_grace_seconds: i64,
    ) -> i64;

    fn prodex_runtime_broker_termination_outcome_plan_v1(
        abi_version: i64,
        termination_outcome: i64,
    ) -> i64;
}

#[allow(clippy::too_many_arguments)]
pub fn process_identity_plan(
    process_absence_proven: bool,
    expected_birth_present: bool,
    birth_present: bool,
    birth_matches: bool,
    path_check_enabled: bool,
    path_present: bool,
    path_matches: bool,
    recheck_enabled: bool,
    recheck_present: bool,
    recheck_matches: bool,
) -> Result<BrokerProcessIdentityPlan, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_process_identity_plan_v1(
            ABI_VERSION,
            i64::from(process_absence_proven),
            i64::from(expected_birth_present),
            i64::from(birth_present),
            i64::from(birth_matches),
            i64::from(path_check_enabled),
            i64::from(path_present),
            i64::from(path_matches),
            i64::from(recheck_enabled),
            i64::from(recheck_present),
            i64::from(recheck_matches),
        )
    };
    match output {
        0 => Ok(BrokerProcessIdentityPlan::Absent),
        1 => Ok(BrokerProcessIdentityPlan::Proven),
        2 => Ok(BrokerProcessIdentityPlan::OwnershipChanged),
        3 => Ok(BrokerProcessIdentityPlan::OwnershipUnproven),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn termination_signal_plan(
    identity: BrokerProcessIdentityPlan,
) -> Result<BrokerTerminationSignalAction, MojoError> {
    let output =
        unsafe { prodex_runtime_broker_termination_signal_plan_v1(ABI_VERSION, identity as i64) };
    match output {
        0 => Ok(BrokerTerminationSignalAction::Skip),
        1 => Ok(BrokerTerminationSignalAction::Signal),
        2 => Ok(BrokerTerminationSignalAction::Refuse),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn lease_lifecycle_plan(
    operation: BrokerLeaseLifecycleOperation,
    pid_valid: bool,
    process_absence_proven: bool,
    process_alive: bool,
    active_requests: usize,
    expired: bool,
) -> Result<BrokerLeaseLifecycleAction, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_lease_lifecycle_plan_v1(
            ABI_VERSION,
            operation as i64,
            i64::from(pid_valid),
            i64::from(process_absence_proven),
            i64::from(process_alive),
            u64::try_from(active_requests).map_err(|_| MojoError::InvalidInput)?,
            i64::from(expired),
        )
    };
    match output {
        0 => Ok(BrokerLeaseLifecycleAction::Ignore),
        1 => Ok(BrokerLeaseLifecycleAction::Keep),
        2 => Ok(BrokerLeaseLifecycleAction::Remove),
        3 => Ok(BrokerLeaseLifecycleAction::Acquire),
        4 => Ok(BrokerLeaseLifecycleAction::Renew),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn registry_process_plan(
    identity: BrokerProcessIdentityPlan,
    active_requests: usize,
    live_leases: usize,
) -> Result<BrokerRegistryProcessAction, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_registry_process_plan_v1(
            ABI_VERSION,
            identity as i64,
            u64::try_from(active_requests).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(live_leases).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match output {
        0 => Ok(BrokerRegistryProcessAction::Keep),
        1 => Ok(BrokerRegistryProcessAction::Remove),
        2 => Ok(BrokerRegistryProcessAction::DiscardStale),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn readiness_plan(
    registry_present: bool,
    instance_matches: bool,
    health_present: bool,
    health_matches: bool,
    elapsed_ms: u64,
    timeout_ms: u64,
) -> Result<BrokerReadinessDecision, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_readiness_plan_v1(
            ABI_VERSION,
            i64::from(registry_present),
            i64::from(instance_matches),
            i64::from(health_present),
            i64::from(health_matches),
            elapsed_ms,
            timeout_ms,
        )
    };
    match output {
        0 => Ok(BrokerReadinessDecision::Wait),
        1 => Ok(BrokerReadinessDecision::Ready),
        2 => Ok(BrokerReadinessDecision::Timeout),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn idle_plan(
    startup_grace_elapsed: bool,
    active_requests: usize,
    live_leases: usize,
    idle_elapsed_seconds: i64,
    idle_grace_seconds: i64,
) -> Result<BrokerIdleDecision, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_idle_plan_v1(
            ABI_VERSION,
            i64::from(startup_grace_elapsed),
            u64::try_from(active_requests).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(live_leases).map_err(|_| MojoError::InvalidInput)?,
            idle_elapsed_seconds,
            idle_grace_seconds,
        )
    };
    match output {
        0 => Ok(BrokerIdleDecision::Wait),
        1 => Ok(BrokerIdleDecision::Reset),
        2 => Ok(BrokerIdleDecision::Shutdown),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn termination_outcome_plan(
    termination_outcome: i64,
) -> Result<BrokerTerminationOutcomePlan, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_termination_outcome_plan_v1(ABI_VERSION, termination_outcome)
    };
    match output {
        1 => Ok(BrokerTerminationOutcomePlan::Cleanup),
        2 => Ok(BrokerTerminationOutcomePlan::Failure),
        3 => Ok(BrokerTerminationOutcomePlan::DiscardStale),
        _ => Err(MojoError::InvalidOutput),
    }
}
