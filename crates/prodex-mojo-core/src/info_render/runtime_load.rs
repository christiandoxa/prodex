use super::{InfoRunwayObservation, InfoStringView, MojoError};

/// Timestamp/window facts for the compiled runtime-load report.
#[derive(Debug, Clone, Copy)]
pub struct InfoRuntimeLoadConfig {
    pub now: i64,
    pub recent_window_seconds: i64,
    pub lookback_seconds: i64,
}

/// A profile's observed in-flight count within one source log.
#[derive(Debug, Clone, Copy)]
pub struct InfoRuntimeInflight<'a> {
    pub log: usize,
    pub profile: &'a str,
    pub units: usize,
}

/// Aggregated counters and observation indices selected by Mojo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoRuntimeLoadPlan {
    pub observation_indices: Vec<usize>,
    pub active_inflight_units: usize,
    pub recent_selection_events: usize,
    pub recent_first_timestamp: Option<i64>,
    pub recent_last_timestamp: Option<i64>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LoadRow {
    group: u64,
    timestamp: i64,
    units: u64,
    profile: InfoStringView,
}
const _: () = assert!(std::mem::size_of::<LoadRow>() == 40);

unsafe extern "C" {
    fn prodex_info_runtime_load_plan_v1(
        abi: i64,
        observations: u64,
        observation_count: i64,
        inflight: u64,
        inflight_count: i64,
        now: i64,
        recent_seconds: i64,
        lookback_seconds: i64,
        output: u64,
        output_words: i64,
    ) -> i64;
}

/// Selects latest per-log profile counts and filters/orders observations in the report owner.
pub fn info_runtime_load_plan(
    observations: &[InfoRunwayObservation<'_>],
    inflight: &[InfoRuntimeInflight<'_>],
    config: InfoRuntimeLoadConfig,
) -> Result<InfoRuntimeLoadPlan, MojoError> {
    let observations = observations
        .iter()
        .map(|row| LoadRow {
            group: 0,
            timestamp: row.timestamp,
            units: 0,
            profile: InfoStringView {
                ptr: row.profile.as_ptr() as u64,
                len: row.profile.len() as u64,
            },
        })
        .collect::<Vec<_>>();
    let inflight = inflight
        .iter()
        .map(|row| LoadRow {
            group: row.log as u64,
            timestamp: 0,
            units: row.units as u64,
            profile: InfoStringView {
                ptr: row.profile.as_ptr() as u64,
                len: row.profile.len() as u64,
            },
        })
        .collect::<Vec<_>>();
    let capacity = 5_usize
        .checked_add(observations.len())
        .and_then(|n| n.checked_add(inflight.len()))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u64; capacity];
    let status = unsafe {
        prodex_info_runtime_load_plan_v1(
            1,
            observations.as_ptr() as u64,
            i64::try_from(observations.len()).map_err(|_| MojoError::InvalidInput)?,
            inflight.as_ptr() as u64,
            i64::try_from(inflight.len()).map_err(|_| MojoError::InvalidInput)?,
            config.now,
            config.recent_window_seconds,
            config.lookback_seconds,
            output.as_mut_ptr() as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let recent = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if count > observations.len() || recent > observations.len() {
        return Err(MojoError::InvalidOutput);
    }
    let indices = output[5..5 + count]
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).map_err(|_| MojoError::InvalidOutput)?;
            if index >= observations.len() {
                return Err(MojoError::InvalidOutput);
            }
            Ok(index)
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    Ok(InfoRuntimeLoadPlan {
        observation_indices: indices,
        active_inflight_units: usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        recent_selection_events: recent,
        recent_first_timestamp: (recent > 0).then_some(output[3] as i64),
        recent_last_timestamp: (recent > 0).then_some(output[4] as i64),
    })
}

#[cfg(test)]
#[path = "runtime_load_tests.rs"]
mod tests;
