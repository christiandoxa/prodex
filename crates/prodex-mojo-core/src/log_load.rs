use crate::MojoError;

const ABI_VERSION: i64 = 1;
const OPERATION_UPDATE: i64 = 0;
const OPERATION_SUMMARY: i64 = 1;
const MAX_UNIQUE_RUNS: usize = 256;
const MAX_EVENT_NAME_BYTES: usize = 256;
const MAX_KEY_BYTES: usize = 16_384;
const MAX_RUN_ID_BYTES: usize = 256;
const UPDATE_OUTPUT_FIELDS: usize = 5;
const SUMMARY_OUTPUT_CAPACITY: usize = 64;

const _: () = assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());

unsafe extern "C" {
    fn prodex_mojo_log_load_semantics_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// State and monotonic-time inputs for one bounded load-event aggregation decision.
pub struct LogLoadAggregateInput<'a> {
    /// Runtime event name to classify.
    pub event_name: &'a str,
    /// Existing aggregate key, or `None` when starting a new aggregate.
    pub previous_key: Option<&'a str>,
    /// Key for the incoming observation.
    pub observation_key: &'a str,
    /// Monotonic elapsed time since the previous observation, in nanoseconds.
    pub elapsed_ns: u64,
    /// Existing aggregate occurrence count, or zero when no aggregate exists.
    pub occurrences: u64,
    /// Existing unique run IDs, bounded to 256 entries by the Mojo policy.
    pub unique_run_ids: &'a [String],
    /// Whether the existing unique-run count exceeded its bound.
    pub run_count_overflow: bool,
    /// Run ID for the incoming observation, if present.
    pub run_id: Option<&'a str>,
}

/// Authoritative routine-event and bounded aggregate update decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogLoadAggregatePlan {
    /// Whether the event is routine telemetry hidden from the human timeline.
    pub routine: bool,
    /// Whether the incoming observation extends the existing aggregate.
    pub coalesce: bool,
    /// Occurrence count after applying the observation.
    pub occurrences: u64,
    /// Whether Rust should append the incoming run ID to its bounded vector.
    pub append_run: bool,
    /// Overflow state after applying the observation.
    pub run_count_overflow: bool,
}

/// Apply Mojo's classification, freshness, deduplication, and saturation plan.
pub fn aggregate_update(
    input: LogLoadAggregateInput<'_>,
) -> Result<LogLoadAggregatePlan, MojoError> {
    if input.event_name.len() > MAX_EVENT_NAME_BYTES
        || input.observation_key.len() > MAX_KEY_BYTES
        || input
            .previous_key
            .is_some_and(|key| key.len() > MAX_KEY_BYTES)
        || input.unique_run_ids.len() > MAX_UNIQUE_RUNS
        || (input.run_count_overflow && input.unique_run_ids.len() != MAX_UNIQUE_RUNS)
        || input
            .unique_run_ids
            .iter()
            .any(|run_id| run_id.len() > MAX_RUN_ID_BYTES)
        || input
            .run_id
            .is_some_and(|run_id| run_id.len() > MAX_RUN_ID_BYTES)
        || (input.previous_key.is_none()
            && (input.occurrences != 0
                || !input.unique_run_ids.is_empty()
                || input.run_count_overflow))
        || (input.previous_key.is_some() && input.occurrences == 0)
    {
        return Err(MojoError::InvalidInput);
    }

    let mut run_id_views = Vec::with_capacity(input.unique_run_ids.len() * 2);
    for run_id in input.unique_run_ids {
        run_id_views.push(pointer_address(run_id.as_ptr()));
        run_id_views.push(u64::try_from(run_id.len()).map_err(|_| MojoError::InvalidInput)?);
    }
    let (previous_key_address, previous_key_length) = input
        .previous_key
        .map(|key| (pointer_address(key.as_ptr()), key.len() as u64))
        .unwrap_or_default();
    let (run_id_address, run_id_length, run_id_present) = input
        .run_id
        .map(|run_id| (pointer_address(run_id.as_ptr()), run_id.len() as u64, 1))
        .unwrap_or_default();
    let input_values = [
        pointer_address(input.event_name.as_ptr()),
        input.event_name.len() as u64,
        previous_key_address,
        previous_key_length,
        pointer_address(input.observation_key.as_ptr()),
        input.observation_key.len() as u64,
        input.elapsed_ns,
        input.occurrences,
        u64::from(input.run_count_overflow),
        run_id_address,
        run_id_length,
        run_id_present,
        pointer_address(run_id_views.as_ptr()),
        input.unique_run_ids.len() as u64,
        u64::from(input.previous_key.is_some()),
    ];
    let mut output = [u64::MAX; UPDATE_OUTPUT_FIELDS];
    status(unsafe {
        prodex_mojo_log_load_semantics_v1(
            ABI_VERSION,
            OPERATION_UPDATE,
            pointer_address(input_values.as_ptr()),
            mutable_pointer_address(output.as_mut_ptr()),
            UPDATE_OUTPUT_FIELDS as i64,
            0,
        )
    })?;
    Ok(LogLoadAggregatePlan {
        routine: bool_output(output[0])?,
        coalesce: bool_output(output[1])?,
        occurrences: output[2],
        append_run: bool_output(output[3])?,
        run_count_overflow: bool_output(output[4])?,
    })
}

/// Format the human-facing aggregate suffix through the same required Mojo owner.
pub fn aggregate_summary(
    occurrences: usize,
    unique_run_count: usize,
    run_count_overflow: bool,
) -> Result<String, MojoError> {
    if unique_run_count > MAX_UNIQUE_RUNS
        || (run_count_overflow && unique_run_count != MAX_UNIQUE_RUNS)
    {
        return Err(MojoError::InvalidInput);
    }
    let input = [
        u64::try_from(occurrences).map_err(|_| MojoError::InvalidInput)?,
        unique_run_count as u64,
        u64::from(run_count_overflow),
    ];
    let mut output = [0_u8; SUMMARY_OUTPUT_CAPACITY];
    let mut written = -1_i64;
    status(unsafe {
        prodex_mojo_log_load_semantics_v1(
            ABI_VERSION,
            OPERATION_SUMMARY,
            pointer_address(input.as_ptr()),
            mutable_pointer_address(output.as_mut_ptr()),
            SUMMARY_OUTPUT_CAPACITY as i64,
            mutable_pointer_address(&mut written),
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

/// Classify routine load telemetry through the authoritative Mojo operation.
pub fn is_routine_event(event_name: &str) -> bool {
    aggregate_update(LogLoadAggregateInput {
        event_name,
        previous_key: None,
        observation_key: "",
        elapsed_ns: 0,
        occurrences: 0,
        unique_run_ids: &[],
        run_count_overflow: false,
        run_id: None,
    })
    .unwrap_or_else(|error| panic!("Mojo log-load classification failed: {error:?}"))
    .routine
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn bool_output(value: u64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}
