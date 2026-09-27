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
    }
}
