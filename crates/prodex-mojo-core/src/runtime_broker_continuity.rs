use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuityEvent {
    ChainRetriedOwner,
    ChainDeadUpstreamConfirmed,
    StaleContinuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuityReasonSource {
    DirectReason,
    Message,
    RawLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuityLinePlan {
    pub event: Option<ContinuityEvent>,
    pub reason_source: Option<ContinuityReasonSource>,
    pub reason_start: usize,
    pub reason_length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuityRouteKind {
    Unknown,
    Responses,
    Compact,
    Websocket,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthKeyKind {
    Ignore,
    Route,
    Profile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum BrokerRegistryArtifactStatus {
    NotLegacy = 0,
    ValidLegacy = 1,
    Malformed = 2,
    TooLarge = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerRegistryStoreAction {
    ReadCurrent,
    RemoveLegacyArtifacts,
    RemoveLegacyBackup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerRegistryErrorSource {
    None,
    Primary,
    Backup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerRegistryStorePlan {
    pub action: BrokerRegistryStoreAction,
    pub error_source: BrokerRegistryErrorSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerLogFingerprintRelation {
    Rebuild,
    Exact,
    Append,
    Rotated,
}

#[derive(Debug, Clone, Copy)]
pub struct BrokerBinaryIdentityView<'a> {
    pub version: Option<&'a str>,
    pub sha256: Option<&'a str>,
    pub path_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerReplacementReason {
    Sha256Mismatch,
    VersionMismatch,
    IdentityMismatch,
    IdentityUnresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerVersionGuardOutcome {
    Compatible,
    DeferredActiveRequests,
    Replaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerVersionGuardPlan {
    pub outcome: BrokerVersionGuardOutcome,
    pub use_version_identity: bool,
    pub replacement_reason: Option<BrokerReplacementReason>,
}

unsafe extern "C" {
    fn prodex_runtime_broker_continuity_line_v1(
        abi_version: i64,
        raw_address: u64,
        raw_length: i64,
        event_present: i64,
        event_address: u64,
        event_length: i64,
        reason_present: i64,
        reason_address: u64,
        reason_length: i64,
        message_present: i64,
        message_address: u64,
        message_length: i64,
        output_address: u64,
    ) -> i64;

    fn prodex_runtime_broker_effective_score_v1(
        abi_version: i64,
        score: i64,
        updated_at: i64,
        now: i64,
        decay_seconds: i64,
    ) -> i64;

    fn prodex_runtime_broker_stale_verified_v1(
        abi_version: i64,
        verified: i64,
        not_found_present: i64,
        last_not_found_at: i64,
        verified_present: i64,
        last_verified_at: i64,
        touched_present: i64,
        last_touched_at: i64,
        now: i64,
        stale_verified_seconds: i64,
    ) -> i64;

    fn prodex_runtime_broker_route_kind_v1(
        abi_version: i64,
        route_address: u64,
        route_length: i64,
    ) -> i64;

    fn prodex_runtime_broker_health_key_kind_v1(
        abi_version: i64,
        key_address: u64,
        key_length: i64,
    ) -> i64;

    fn prodex_runtime_broker_registry_store_plan_v1(
        abi_version: i64,
        primary_exists: i64,
        primary_status: i64,
        backup_status: i64,
        primary_current: i64,
        output_address: u64,
    ) -> i64;

    fn prodex_runtime_broker_registry_identity_match_v1(
        abi_version: i64,
        left_address: u64,
        left_length: i64,
        right_address: u64,
        right_length: i64,
    ) -> i64;

    fn prodex_runtime_broker_identity_policy_v1(
        abi_version: i64,
        mode: i64,
        left_flags: i64,
        left_version_address: u64,
        left_version_length: i64,
        left_sha_address: u64,
        left_sha_length: i64,
        right_flags: i64,
        right_version_address: u64,
        right_version_length: i64,
        right_sha_address: u64,
        right_sha_length: i64,
    ) -> i64;

    fn prodex_runtime_broker_guard_plan_v1(
        abi_version: i64,
        process_alive: i64,
        binary_flags: i64,
        binary_version_address: u64,
        binary_version_length: i64,
        binary_sha_address: u64,
        binary_sha_length: i64,
        version_flags: i64,
        version_version_address: u64,
        version_version_length: i64,
        version_sha_address: u64,
        version_sha_length: i64,
        observed_flags: i64,
        observed_version_address: u64,
        observed_version_length: i64,
        observed_sha_address: u64,
        observed_sha_length: i64,
        active_requests: u64,
        live_leases: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_runtime_broker_parse_version_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;

    fn prodex_runtime_broker_log_cache_relation_v1(
        abi_version: i64,
        current_len: u64,
        current_modified_seconds: u64,
        current_modified_nanoseconds: u64,
        previous_present: i64,
        previous_len: u64,
        previous_modified_seconds: u64,
        previous_modified_nanoseconds: u64,
    ) -> i64;

    fn prodex_runtime_broker_lru_evict_index_v1(
        abi_version: i64,
        touches_address: u64,
        count: i64,
        keep_index: i64,
    ) -> i64;
}

fn ptr(value: &str) -> u64 {
    value.as_ptr() as usize as u64
}

fn length(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn optional_parts(value: Option<&str>) -> Result<(i64, u64, i64), MojoError> {
    match value {
        Some(value) => Ok((1, ptr(value), length(value)?)),
        None => Ok((0, 0, 0)),
    }
}

pub fn continuity_line_plan(
    raw_line: &str,
    event: Option<&str>,
    reason: Option<&str>,
    message: Option<&str>,
) -> Result<ContinuityLinePlan, MojoError> {
    let (event_present, event_address, event_length) = optional_parts(event)?;
    let (reason_present, reason_address, reason_length) = optional_parts(reason)?;
    let (message_present, message_address, message_length) = optional_parts(message)?;
    let mut output = [0_i64; 4];
    let status = unsafe {
        prodex_runtime_broker_continuity_line_v1(
            ABI_VERSION,
            ptr(raw_line),
            length(raw_line)?,
            event_present,
            event_address,
            event_length,
            reason_present,
            reason_address,
            reason_length,
            message_present,
            message_address,
            message_length,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    let event = match output[0] {
        0 => None,
        1 => Some(ContinuityEvent::ChainRetriedOwner),
        2 => Some(ContinuityEvent::ChainDeadUpstreamConfirmed),
        3 => Some(ContinuityEvent::StaleContinuation),
        _ => return Err(MojoError::InvalidOutput),
    };
    let reason_source = match output[1] {
        0 => None,
        1 => Some(ContinuityReasonSource::DirectReason),
        2 => Some(ContinuityReasonSource::Message),
        3 => Some(ContinuityReasonSource::RawLine),
        _ => return Err(MojoError::InvalidOutput),
    };
    let (reason_start, reason_length) = if reason_source.is_some() {
        (
            usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?,
            usize::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?,
        )
    } else {
        (0, 0)
    };
    Ok(ContinuityLinePlan {
        event,
        reason_source,
        reason_start,
        reason_length,
    })
}

pub fn effective_score(
    score: u32,
    updated_at: i64,
    now: i64,
    decay_seconds: i64,
) -> Result<u32, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_effective_score_v1(
            ABI_VERSION,
            i64::from(score),
            updated_at,
            now,
            decay_seconds,
        )
    };
    u32::try_from(output).map_err(|_| MojoError::InvalidOutput)
}

pub fn stale_verified(
    verified: bool,
    last_not_found_at: Option<i64>,
    last_verified_at: Option<i64>,
    last_touched_at: Option<i64>,
    now: i64,
    stale_verified_seconds: i64,
) -> Result<bool, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_stale_verified_v1(
            ABI_VERSION,
            i64::from(verified),
            i64::from(last_not_found_at.is_some()),
            last_not_found_at.unwrap_or_default(),
            i64::from(last_verified_at.is_some()),
            last_verified_at.unwrap_or_default(),
            i64::from(last_touched_at.is_some()),
            last_touched_at.unwrap_or_default(),
            now,
            stale_verified_seconds,
        )
    };
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn route_kind(route: &str) -> Result<ContinuityRouteKind, MojoError> {
    let output =
        unsafe { prodex_runtime_broker_route_kind_v1(ABI_VERSION, ptr(route), length(route)?) };
    match output {
        0 => Ok(ContinuityRouteKind::Unknown),
        1 => Ok(ContinuityRouteKind::Responses),
        2 => Ok(ContinuityRouteKind::Compact),
        3 => Ok(ContinuityRouteKind::Websocket),
        4 => Ok(ContinuityRouteKind::Standard),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn registry_store_plan(
    primary_exists: bool,
    primary_status: BrokerRegistryArtifactStatus,
    backup_status: BrokerRegistryArtifactStatus,
    primary_current: bool,
) -> Result<BrokerRegistryStorePlan, MojoError> {
    let mut output = [0_i64; 2];
    let status = unsafe {
        prodex_runtime_broker_registry_store_plan_v1(
            ABI_VERSION,
            i64::from(primary_exists),
            primary_status as i64,
            backup_status as i64,
            i64::from(primary_current),
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidOutput);
    }
    let action = match output[0] {
        0 => BrokerRegistryStoreAction::ReadCurrent,
        1 => BrokerRegistryStoreAction::RemoveLegacyArtifacts,
        2 => BrokerRegistryStoreAction::RemoveLegacyBackup,
        _ => return Err(MojoError::InvalidOutput),
    };
    let error_source = match output[1] {
        0 => BrokerRegistryErrorSource::None,
        1 => BrokerRegistryErrorSource::Primary,
        2 => BrokerRegistryErrorSource::Backup,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(BrokerRegistryStorePlan {
        action,
        error_source,
    })
}

pub fn registry_instance_matches(left: &str, right: &str) -> Result<bool, MojoError> {
    let output = unsafe {
        prodex_runtime_broker_registry_identity_match_v1(
            ABI_VERSION,
            ptr(left),
            length(left)?,
            ptr(right),
            length(right)?,
        )
    };
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn identity_parts(
    identity: BrokerBinaryIdentityView<'_>,
) -> Result<(i64, u64, i64, u64, i64), MojoError> {
    let mut flags = 0_i64;
    let (version_address, version_length) = match identity.version {
        Some(value) => {
            flags |= 1;
            (ptr(value), length(value)?)
        }
        None => (0, 0),
    };
    if identity.path_present {
        flags |= 2;
    }
    let (sha_address, sha_length) = match identity.sha256 {
        Some(value) => {
            flags |= 4;
            (ptr(value), length(value)?)
        }
        None => (0, 0),
    };
    Ok((
        flags,
        version_address,
        version_length,
        sha_address,
        sha_length,
    ))
}

fn identity_policy(
    mode: i64,
    left: BrokerBinaryIdentityView<'_>,
    right: BrokerBinaryIdentityView<'_>,
) -> Result<i64, MojoError> {
    let (left_flags, left_version_address, left_version_length, left_sha_address, left_sha_length) =
        identity_parts(left)?;
    let (
        right_flags,
        right_version_address,
        right_version_length,
        right_sha_address,
        right_sha_length,
    ) = identity_parts(right)?;
    let value = unsafe {
        prodex_runtime_broker_identity_policy_v1(
            ABI_VERSION,
            mode,
            left_flags,
            left_version_address,
            left_version_length,
            left_sha_address,
            left_sha_length,
            right_flags,
            right_version_address,
            right_version_length,
            right_sha_address,
            right_sha_length,
        )
    };
    (value >= 0)
        .then_some(value)
        .ok_or(MojoError::InvalidOutput)
}

fn replacement_reason_from_code(value: i64) -> Result<BrokerReplacementReason, MojoError> {
    match value {
        1 => Ok(BrokerReplacementReason::Sha256Mismatch),
        2 => Ok(BrokerReplacementReason::VersionMismatch),
        3 => Ok(BrokerReplacementReason::IdentityMismatch),
        4 => Ok(BrokerReplacementReason::IdentityUnresolved),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn binary_identity_present(identity: BrokerBinaryIdentityView<'_>) -> Result<bool, MojoError> {
    match identity_policy(
        0,
        identity,
        BrokerBinaryIdentityView {
            version: None,
            sha256: None,
            path_present: false,
        },
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn binary_identity_matches(
    current: BrokerBinaryIdentityView<'_>,
    other: BrokerBinaryIdentityView<'_>,
) -> Result<bool, MojoError> {
    match identity_policy(1, current, other)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn binary_identity_replacement_reason(
    current: BrokerBinaryIdentityView<'_>,
    observed: BrokerBinaryIdentityView<'_>,
) -> Result<BrokerReplacementReason, MojoError> {
    replacement_reason_from_code(identity_policy(2, current, observed)?)
}

pub fn binary_identity_version_mismatch(
    current: BrokerBinaryIdentityView<'_>,
    observed: BrokerBinaryIdentityView<'_>,
) -> Result<bool, MojoError> {
    match identity_policy(3, current, observed)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn version_guard_plan(
    process_alive: bool,
    current_binary: BrokerBinaryIdentityView<'_>,
    current_version: BrokerBinaryIdentityView<'_>,
    observed: BrokerBinaryIdentityView<'_>,
    active_requests: usize,
    live_leases: usize,
) -> Result<BrokerVersionGuardPlan, MojoError> {
    let (
        binary_flags,
        binary_version_address,
        binary_version_length,
        binary_sha_address,
        binary_sha_length,
    ) = identity_parts(current_binary)?;
    let (
        version_flags,
        version_version_address,
        version_version_length,
        version_sha_address,
        version_sha_length,
    ) = identity_parts(current_version)?;
    let (
        observed_flags,
        observed_version_address,
        observed_version_length,
        observed_sha_address,
        observed_sha_length,
    ) = identity_parts(observed)?;
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_runtime_broker_guard_plan_v1(
            ABI_VERSION,
            i64::from(process_alive),
            binary_flags,
            binary_version_address,
            binary_version_length,
            binary_sha_address,
            binary_sha_length,
            version_flags,
            version_version_address,
            version_version_length,
            version_sha_address,
            version_sha_length,
            observed_flags,
            observed_version_address,
            observed_version_length,
            observed_sha_address,
            observed_sha_length,
            u64::try_from(active_requests).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(live_leases).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidOutput);
    }
    let outcome = match output[0] {
        0 => BrokerVersionGuardOutcome::Compatible,
        1 => BrokerVersionGuardOutcome::DeferredActiveRequests,
        2 => BrokerVersionGuardOutcome::Replaced,
        _ => return Err(MojoError::InvalidOutput),
    };
    let use_version_identity = match output[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let replacement_reason = match output[2] {
        0 => None,
        value => Some(replacement_reason_from_code(value)?),
    };
    Ok(BrokerVersionGuardPlan {
        outcome,
        use_version_identity,
        replacement_reason,
    })
}

pub fn parse_prodex_version(output: &str) -> Result<Option<&str>, MojoError> {
    let mut span = [-1_i64; 2];
    let status = unsafe {
        prodex_runtime_broker_parse_version_v1(
            ABI_VERSION,
            ptr(output),
            length(output)?,
            span.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(None),
        1 => {
            let start = usize::try_from(span[0]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(span[1]).map_err(|_| MojoError::InvalidOutput)?;
            output
                .get(start..end)
                .map(Some)
                .ok_or(MojoError::InvalidOutput)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn continuity_event_kind(event: &str) -> Result<Option<ContinuityEvent>, MojoError> {
    Ok(continuity_line_plan("", Some(event), None, None)?.event)
}

pub fn log_fingerprint_relation(
    current_len: u64,
    current_modified_seconds: u64,
    current_modified_nanoseconds: u32,
    previous: Option<(u64, u64, u32)>,
) -> Result<BrokerLogFingerprintRelation, MojoError> {
    let (previous_present, previous_len, previous_seconds, previous_nanoseconds) = previous
        .map(|(len, seconds, nanoseconds)| (1_i64, len, seconds, u64::from(nanoseconds)))
        .unwrap_or((0_i64, 0, 0, 0));
    let output = unsafe {
        prodex_runtime_broker_log_cache_relation_v1(
            ABI_VERSION,
            current_len,
            current_modified_seconds,
            u64::from(current_modified_nanoseconds),
            previous_present,
            previous_len,
            previous_seconds,
            previous_nanoseconds,
        )
    };
    match output {
        0 => Ok(BrokerLogFingerprintRelation::Rebuild),
        1 => Ok(BrokerLogFingerprintRelation::Exact),
        2 => Ok(BrokerLogFingerprintRelation::Append),
        3 => Ok(BrokerLogFingerprintRelation::Rotated),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn lru_evict_index(
    touches: &[u64],
    keep_index: Option<usize>,
) -> Result<Option<usize>, MojoError> {
    let keep_index = keep_index
        .map(|index| i64::try_from(index).map_err(|_| MojoError::InvalidInput))
        .transpose()?
        .unwrap_or(-1);
    let output = unsafe {
        prodex_runtime_broker_lru_evict_index_v1(
            ABI_VERSION,
            touches.as_ptr() as usize as u64,
            i64::try_from(touches.len()).map_err(|_| MojoError::InvalidInput)?,
            keep_index,
        )
    };
    match output {
        -2 => Ok(None),
        value if value >= 0 => {
            let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
            if index < touches.len() {
                Ok(Some(index))
            } else {
                Err(MojoError::InvalidOutput)
            }
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn health_key_kind(key: &str) -> Result<HealthKeyKind, MojoError> {
    let output =
        unsafe { prodex_runtime_broker_health_key_kind_v1(ABI_VERSION, ptr(key), length(key)?) };
    match output {
        0 => Ok(HealthKeyKind::Ignore),
        1 => Ok(HealthKeyKind::Route),
        2 => Ok(HealthKeyKind::Profile),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_continuity_kernel_smoke() {
        let plan = continuity_line_plan(
            "request=3 chain_retried_owner profile=second reason=\"quoted reason\"",
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(plan.event, Some(ContinuityEvent::ChainRetriedOwner));
        assert_eq!(plan.reason_source, Some(ContinuityReasonSource::RawLine));

        assert_eq!(effective_score(4, 90, 100, 5).unwrap(), 2);
        assert!(stale_verified(true, None, Some(80), None, 100, 10).unwrap());
        assert_eq!(
            route_kind("websocket").unwrap(),
            ContinuityRouteKind::Websocket
        );
        assert_eq!(
            health_key_kind("__route_health__:responses:main").unwrap(),
            HealthKeyKind::Route
        );
        let current = BrokerBinaryIdentityView {
            version: Some("0.7.0"),
            sha256: Some("abc123"),
            path_present: true,
        };
        let same_sha = BrokerBinaryIdentityView {
            version: Some("0.8.0"),
            sha256: Some("abc123"),
            path_present: false,
        };
        let different = BrokerBinaryIdentityView {
            version: Some("0.8.0"),
            sha256: Some("def456"),
            path_present: false,
        };
        assert!(binary_identity_present(current).unwrap());
        assert!(binary_identity_matches(current, same_sha).unwrap());
        assert_eq!(
            binary_identity_replacement_reason(current, different).unwrap(),
            BrokerReplacementReason::Sha256Mismatch
        );
        assert!(binary_identity_version_mismatch(current, different).unwrap());
        assert_eq!(
            version_guard_plan(
                true,
                current,
                BrokerBinaryIdentityView {
                    version: Some("0.7.0"),
                    sha256: None,
                    path_present: false,
                },
                different,
                0,
                0
            )
            .unwrap(),
            BrokerVersionGuardPlan {
                outcome: BrokerVersionGuardOutcome::Replaced,
                use_version_identity: true,
                replacement_reason: Some(BrokerReplacementReason::VersionMismatch),
            }
        );
        assert_eq!(
            parse_prodex_version(
                "  prodex 0.7.0
"
            )
            .unwrap(),
            Some("0.7.0")
        );
        assert_eq!(parse_prodex_version("codex 0.7.0").unwrap(), None);
        assert_eq!(
            continuity_event_kind("chain_dead_upstream_confirmed").unwrap(),
            Some(ContinuityEvent::ChainDeadUpstreamConfirmed)
        );
        assert_eq!(
            log_fingerprint_relation(20, 2, 0, Some((10, 1, 0))).unwrap(),
            BrokerLogFingerprintRelation::Append
        );
        assert_eq!(
            log_fingerprint_relation(9, 2, 0, Some((10, 1, 0))).unwrap(),
            BrokerLogFingerprintRelation::Rotated
        );
        assert_eq!(lru_evict_index(&[5, 2, 9], Some(1)).unwrap(), Some(0));
        assert_eq!(lru_evict_index(&[5], Some(0)).unwrap(), Some(0));
        assert_eq!(lru_evict_index(&[], None).unwrap(), None);
    }
}
