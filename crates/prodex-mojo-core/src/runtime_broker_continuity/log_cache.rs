//! Typed log fingerprint, eviction and health-key ABI projections.
use super::*;

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
