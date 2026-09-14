use crate::MojoError;

pub const GATEWAY_ADMIN_POLICY_ABI_VERSION: i64 = 1;
const MAX_RETENTION_LIMIT: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewayAdminRetentionPlan {
    pub retention_days: u16,
    pub batch_limit: u16,
}

unsafe extern "C" {
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
    }
}
