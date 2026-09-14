use crate::MojoError;
use std::cmp::Ordering;

pub const GATEWAY_ADMIN_POLICY_ABI_VERSION: i64 = 1;
const MAX_RETENTION_LIMIT: u64 = 1_000;
const AUDIT_TIME_RANGE_CONTAINS: i64 = 0;
const AUDIT_COMPARE_POSITIONS: i64 = 1;
const AUDIT_RETENTION_CUTOFF: i64 = 2;
const AUDIT_EVENT_EXPIRED: i64 = 3;
const AUDIT_HOLD_ACTIVE: i64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayAdminRetentionPlan {
    pub retention_days: u16,
    pub batch_limit: u16,
}

unsafe extern "C" {
    fn prodex_mojo_audit_decision_v1(
        abi_version: i64,
        operation: i64,
        values: u64,
        value_count: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_gateway_admin_retention_plan_v1(
        abi_version: i64,
        retention_days: u64,
        retention_present: i64,
        event_count: i64,
        normalized_days: u64,
        batch_limit: u64,
    ) -> i64;
    fn prodex_mojo_gateway_admin_limit_plan_v1(
        abi_version: i64,
        requested_limit: u64,
        requested_present: i64,
        normalized_limit: u64,
    ) -> i64;
    fn prodex_mojo_gateway_admin_cutoff_v1(
        abi_version: i64,
        now_unix_ms: u64,
        retention_days: u64,
        cutoff: u64,
    ) -> i64;
    fn prodex_mojo_gateway_admin_purge_result_v1(
        abi_version: i64,
        requested: u64,
        purged: u64,
        protected_or_ineligible: u64,
    ) -> i64;
}

fn audit_decision(operation: i64, values: &[u64]) -> Result<u64, MojoError> {
    let mut output = 0;
    let status = unsafe {
        prodex_mojo_audit_decision_v1(
            GATEWAY_ADMIN_POLICY_ABI_VERSION,
            operation,
            values.as_ptr() as u64,
            i64::try_from(values.len()).map_err(|_| MojoError::InvalidInput)?,
            pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    Ok(output)
}

pub fn audit_time_range_contains(
    start: Option<u64>,
    end: Option<u64>,
    timestamp: u64,
) -> Result<bool, MojoError> {
    let (start, start_present) = option_input(start);
    let (end, end_present) = option_input(end);
    match audit_decision(
        AUDIT_TIME_RANGE_CONTAINS,
        &[
            u64::from(start_present == 1),
            start,
            u64::from(end_present == 1),
            end,
            timestamp,
        ],
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn compare_audit_positions(
    left_timestamp: u64,
    left_id: [u64; 2],
    right_timestamp: u64,
    right_id: [u64; 2],
    descending: bool,
) -> Result<Ordering, MojoError> {
    match audit_decision(
        AUDIT_COMPARE_POSITIONS,
        &[
            left_timestamp,
            left_id[0],
            left_id[1],
            right_timestamp,
            right_id[0],
            right_id[1],
            u64::from(descending),
        ],
    )? {
        0 => Ok(Ordering::Less),
        1 => Ok(Ordering::Equal),
        2 => Ok(Ordering::Greater),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn audit_retention_cutoff(
    now_unix_ms: u64,
    retention_days: u16,
    minimum_unix_ms: u64,
) -> Result<u64, MojoError> {
    audit_decision(
        AUDIT_RETENTION_CUTOFF,
        &[now_unix_ms, u64::from(retention_days), minimum_unix_ms],
    )
}

pub fn audit_event_is_expired(event_unix_ms: u64, cutoff_unix_ms: u64) -> Result<bool, MojoError> {
    match audit_decision(AUDIT_EVENT_EXPIRED, &[event_unix_ms, cutoff_unix_ms])? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn audit_hold_is_active(expires_at: Option<u64>, now_unix_ms: u64) -> Result<bool, MojoError> {
    let (expires_at, present) = option_input(expires_at);
    match audit_decision(
        AUDIT_HOLD_ACTIVE,
        &[u64::from(present == 1), expires_at, now_unix_ms],
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn status_error(status: i64) -> MojoError {
    match status {
        1 => MojoError::InvalidInput,
        4 => MojoError::AbiMismatch,
        _ => MojoError::InvalidOutput,
    }
}

fn option_input(value: Option<u64>) -> (u64, i64) {
    value.map_or((0, 0), |value| (value, 1))
}

/// Normalize the bounded retention request values used by the gateway admin route.
pub fn plan_gateway_admin_retention(
    retention_days: Option<u64>,
    event_count: usize,
) -> Result<GatewayAdminRetentionPlan, MojoError> {
    let (retention_days, retention_present) = option_input(retention_days);
    let event_count = i64::try_from(event_count).map_err(|_| MojoError::InvalidInput)?;
    let mut normalized_days = 0_u64;
    let mut batch_limit = 0_u64;
    let status = unsafe {
        prodex_mojo_gateway_admin_retention_plan_v1(
            GATEWAY_ADMIN_POLICY_ABI_VERSION,
            retention_days,
            retention_present,
            event_count,
            pointer_address(&mut normalized_days),
            pointer_address(&mut batch_limit),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    Ok(GatewayAdminRetentionPlan {
        retention_days: u16::try_from(normalized_days).map_err(|_| MojoError::InvalidOutput)?,
        batch_limit: u16::try_from(batch_limit).map_err(|_| MojoError::InvalidOutput)?,
    })
}

/// Normalize a bounded audit-export page limit, including its default.
pub fn plan_gateway_admin_limit(requested_limit: Option<u64>) -> Result<u16, MojoError> {
    let (requested_limit, requested_present) = option_input(requested_limit);
    let mut normalized_limit = 0_u64;
    let status = unsafe {
        prodex_mojo_gateway_admin_limit_plan_v1(
            GATEWAY_ADMIN_POLICY_ABI_VERSION,
            requested_limit,
            requested_present,
            pointer_address(&mut normalized_limit),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    let normalized_limit = u16::try_from(normalized_limit).map_err(|_| MojoError::InvalidOutput)?;
    (normalized_limit > 0 && u64::from(normalized_limit) <= MAX_RETENTION_LIMIT)
        .then_some(normalized_limit)
        .ok_or(MojoError::InvalidOutput)
}

/// Calculate the tenant audit-retention cutoff without carrying tenant data over FFI.
pub fn gateway_admin_retention_cutoff(
    now_unix_ms: u64,
    retention_days: u16,
) -> Result<u64, MojoError> {
    let mut cutoff = 0_u64;
    let status = unsafe {
        prodex_mojo_gateway_admin_cutoff_v1(
            GATEWAY_ADMIN_POLICY_ABI_VERSION,
            now_unix_ms,
            u64::from(retention_days),
            pointer_address(&mut cutoff),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    Ok(cutoff)
}

/// Plan the redacted count shown after a retention purge.
pub fn gateway_admin_purge_protected_count(
    requested: usize,
    purged: usize,
) -> Result<usize, MojoError> {
    let requested = u64::try_from(requested).map_err(|_| MojoError::InvalidInput)?;
    let purged = u64::try_from(purged).map_err(|_| MojoError::InvalidInput)?;
    let mut protected_or_ineligible = 0_u64;
    let status = unsafe {
        prodex_mojo_gateway_admin_purge_result_v1(
            GATEWAY_ADMIN_POLICY_ABI_VERSION,
            requested,
            purged,
            pointer_address(&mut protected_or_ineligible),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    usize::try_from(protected_or_ineligible).map_err(|_| MojoError::InvalidOutput)
}

#[inline]
fn pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_admin_policy_matches_bounded_contract() {
        assert_eq!(
            plan_gateway_admin_retention(None, 2),
            Ok(GatewayAdminRetentionPlan {
                retention_days: 365,
                batch_limit: 2,
            })
        );
        assert_eq!(
            plan_gateway_admin_retention(Some(30), 1),
            Ok(GatewayAdminRetentionPlan {
                retention_days: 30,
                batch_limit: 1,
            })
        );
        assert!(plan_gateway_admin_retention(Some(29), 1).is_err());
        assert!(plan_gateway_admin_retention(Some(3_651), 1).is_err());
        assert!(plan_gateway_admin_retention(Some(365), 0).is_err());

        assert_eq!(plan_gateway_admin_limit(None), Ok(100));
        assert_eq!(plan_gateway_admin_limit(Some(1_000)), Ok(1_000));
        assert!(plan_gateway_admin_limit(Some(0)).is_err());
        assert!(plan_gateway_admin_limit(Some(1_001)).is_err());

        assert_eq!(gateway_admin_retention_cutoff(100_000, 30), Ok(0));
        assert_eq!(gateway_admin_purge_protected_count(7, 3), Ok(4));
        assert!(gateway_admin_purge_protected_count(3, 4).is_err());

        assert_eq!(audit_time_range_contains(Some(10), Some(20), 10), Ok(true));
        assert_eq!(audit_time_range_contains(Some(10), Some(20), 21), Ok(false));
        assert_eq!(
            compare_audit_positions(20, [0, 1], 10, [0, 2], true),
            Ok(Ordering::Less)
        );
        assert_eq!(
            compare_audit_positions(10, [0, 2], 10, [0, 1], true),
            Ok(Ordering::Greater)
        );
        assert_eq!(audit_retention_cutoff(1_000, 30, 1_000), Ok(1_000));
        assert_eq!(audit_event_is_expired(999, 1_000), Ok(true));
        assert_eq!(audit_hold_is_active(None, 20), Ok(true));
        assert_eq!(audit_hold_is_active(Some(20), 21), Ok(false));
    }
}
