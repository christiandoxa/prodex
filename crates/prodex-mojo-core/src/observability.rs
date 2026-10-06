use crate::MojoError;

pub const OBSERVABILITY_LABEL_ABI_VERSION: i64 = 1;
const OBSERVABILITY_LABEL_MAX_BYTES: usize = 128;

pub const OPERATIONAL_EVENT_SOURCE_NONE: i64 = 0;
pub const OPERATIONAL_EVENT_SOURCE_REQUEST: i64 = 1;
pub const OPERATIONAL_EVENT_SOURCE_MCP: i64 = 2;
pub const OPERATIONAL_EVENT_SOURCE_AGENT: i64 = 3;
pub const OPERATIONAL_EVENT_SOURCE_ROUTE: i64 = 4;
pub const OPERATIONAL_EVENT_SOURCE_QUOTA: i64 = 5;
pub const OPERATIONAL_EVENT_SOURCE_RETRY: i64 = 6;
pub const OPERATIONAL_EVENT_SOURCE_BACKOFF: i64 = 7;
pub const OPERATIONAL_EVENT_SOURCE_HEALTH: i64 = 8;
pub const OPERATIONAL_EVENT_SOURCE_ERROR: i64 = 9;
pub const OPERATIONAL_EVENT_SOURCE_MODEL: i64 = 10;
pub const OPERATIONAL_EVENT_SOURCE_UPSTREAM: i64 = 11;
pub const OPERATIONAL_EVENT_SOURCE_STREAM: i64 = 12;
pub const OPERATIONAL_EVENT_SOURCE_RESPONSE: i64 = 13;
pub const OPERATIONAL_EVENT_SOURCE_TERMINAL: i64 = 14;
pub const OPERATIONAL_EVENT_SOURCE_TOOL: i64 = 15;
pub const OPERATIONAL_EVENT_SOURCE_LOAD: i64 = 16;
pub const OPERATIONAL_EVENT_SOURCE_SMART: i64 = 17;
pub const OPERATIONAL_EVENT_SOURCE_COMPACT: i64 = 18;
pub const OPERATIONAL_EVENT_SOURCE_EVENT: i64 = 19;

/// The result of checking a metric key and value against label privacy rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryMetricLabelValidation {
    Valid,
    InvalidKey,
    InvalidValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationalEventPlan {
    pub source: i64,
    pub interesting: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalDetailFormat {
    Plain,
    Percent,
    Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationalDetailSpec {
    pub key: String,
    pub label: String,
    pub format: OperationalDetailFormat,
}

unsafe extern "C" {
    fn prodex_mojo_observability_label_v1(
        abi_version: i64,
        kind: i64,
        value: i64,
        output: u64,
        output_capacity: i64,
        output_length: u64,
    ) -> i64;
    fn prodex_mojo_observability_metric_name_v1(
        abi_version: i64,
        plan: i64,
        slot: i64,
        output: u64,
        output_capacity: i64,
        output_length: u64,
    ) -> i64;
    fn prodex_mojo_observability_label_key_v1(
        abi_version: i64,
        key: i64,
        output: u64,
        output_capacity: i64,
        output_length: u64,
    ) -> i64;
    fn prodex_mojo_observability_metric_label_validate_v1(
        abi_version: i64,
        key_address: u64,
        key_length: i64,
        value_address: u64,
        value_length: i64,
        output_tag: u64,
    ) -> i64;
    fn prodex_mojo_operational_event_plan_v1(
        abi_version: i64,
        event_address: u64,
        event_length: i64,
        tool_surface_address: u64,
        tool_surface_length: i64,
        tool_surface_present: i64,
        continuation_address: u64,
        continuation_length: i64,
        continuation_present: i64,
        family_address: u64,
        family_length: i64,
        family_present: i64,
        decision_address: u64,
        decision_length: i64,
        decision_present: i64,
        source: u64,
        interesting: u64,
    ) -> i64;
    fn prodex_mojo_operational_event_source_label_v1(
        abi_version: i64,
        source: i64,
        output: u64,
        output_capacity: i64,
        output_length: u64,
    ) -> i64;
    fn prodex_mojo_operational_event_detail_plan_v1(
        abi_version: i64,
        source_address: u64,
        source_length: i64,
        first_local_chunk: i64,
        output_address: u64,
        output_capacity: i64,
        output_count_address: u64,
    ) -> i64;
    fn prodex_mojo_operational_detail_spec_v1(
        abi_version: i64,
        detail: i64,
        key_output_address: u64,
        key_output_capacity: i64,
        key_output_length_address: u64,
        label_output_address: u64,
        label_output_capacity: i64,
        label_output_length_address: u64,
        format_address: u64,
    ) -> i64;
}

fn optional_text_parts(value: Option<&str>) -> Result<(u64, i64, i64), MojoError> {
    match value {
        Some(value) => Ok((
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            1,
        )),
        None => Ok((0, 0, 0)),
    }
}

pub fn operational_event_plan(
    event: &str,
    tool_surface: Option<&str>,
    continuation: Option<&str>,
    family: Option<&str>,
    decision: Option<&str>,
) -> Result<OperationalEventPlan, MojoError> {
    if event.is_empty() {
        return Err(MojoError::InvalidInput);
    }
    let event_length = i64::try_from(event.len()).map_err(|_| MojoError::InvalidInput)?;
    let (tool_surface_address, tool_surface_length, tool_surface_present) =
        optional_text_parts(tool_surface)?;
    let (continuation_address, continuation_length, continuation_present) =
        optional_text_parts(continuation)?;
    let (family_address, family_length, family_present) = optional_text_parts(family)?;
    let (decision_address, decision_length, decision_present) = optional_text_parts(decision)?;
    let mut source = -1_i64;
    let mut interesting = -1_i64;
    let status = unsafe {
        prodex_mojo_operational_event_plan_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            event.as_ptr() as usize as u64,
            event_length,
            tool_surface_address,
            tool_surface_length,
            tool_surface_present,
            continuation_address,
            continuation_length,
            continuation_present,
            family_address,
            family_length,
            family_present,
            decision_address,
            decision_length,
            decision_present,
            (&mut source as *mut i64) as usize as u64,
            (&mut interesting as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    if !(OPERATIONAL_EVENT_SOURCE_NONE..=OPERATIONAL_EVENT_SOURCE_EVENT).contains(&source)
        || !matches!(interesting, 0 | 1)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(OperationalEventPlan {
        source,
        interesting: interesting == 1,
    })
}

fn load_operational_event_source_label(source: i64) -> Result<Option<String>, MojoError> {
    if !(OPERATIONAL_EVENT_SOURCE_NONE..=OPERATIONAL_EVENT_SOURCE_EVENT).contains(&source) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut output_length = -2_i64;
    let status = unsafe {
        prodex_mojo_operational_event_source_label_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            source,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output_length as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if output_length == -1 {
        return Ok(None);
    }
    let output_length = usize::try_from(output_length).map_err(|_| MojoError::InvalidOutput)?;
    if output_length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..output_length].to_vec())
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

pub fn operational_event_source_label(source: i64) -> Result<Option<&'static str>, MojoError> {
    use std::sync::OnceLock;

    static LABELS: OnceLock<Result<Vec<Option<String>>, MojoError>> = OnceLock::new();

    let source = usize::try_from(source)
        .ok()
        .filter(|source| *source <= OPERATIONAL_EVENT_SOURCE_EVENT as usize)
        .ok_or(MojoError::InvalidInput)?;
    match LABELS.get_or_init(|| {
        (OPERATIONAL_EVENT_SOURCE_NONE..=OPERATIONAL_EVENT_SOURCE_EVENT)
            .map(load_operational_event_source_label)
            .collect()
    }) {
        Ok(labels) => Ok(labels
            .get(source)
            .ok_or(MojoError::InvalidOutput)?
            .as_deref()),
        Err(error) => Err(*error),
    }
}

fn load_operational_detail_spec(detail: i64) -> Result<OperationalDetailSpec, MojoError> {
    if !(0..=61).contains(&detail) {
        return Err(MojoError::InvalidInput);
    }
    let mut key = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut key_length = -1_i64;
    let mut label = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut label_length = -1_i64;
    let mut format = -1_i64;
    let status = unsafe {
        prodex_mojo_operational_detail_spec_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            detail,
            key.as_mut_ptr() as usize as u64,
            i64::try_from(key.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut key_length as *mut i64) as usize as u64,
            label.as_mut_ptr() as usize as u64,
            i64::try_from(label.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut label_length as *mut i64) as usize as u64,
            (&mut format as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let key_length = usize::try_from(key_length).map_err(|_| MojoError::InvalidOutput)?;
    let label_length = usize::try_from(label_length).map_err(|_| MojoError::InvalidOutput)?;
    if key_length > key.len() || label_length > label.len() {
        return Err(MojoError::InvalidOutput);
    }
    let key =
        String::from_utf8(key[..key_length].to_vec()).map_err(|_| MojoError::InvalidOutput)?;
    let label =
        String::from_utf8(label[..label_length].to_vec()).map_err(|_| MojoError::InvalidOutput)?;
    let format = match format {
        0 => OperationalDetailFormat::Plain,
        1 => OperationalDetailFormat::Percent,
        2 => OperationalDetailFormat::Endpoint,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(OperationalDetailSpec { key, label, format })
}

pub fn operational_detail_spec(detail: i64) -> Result<&'static OperationalDetailSpec, MojoError> {
    use std::sync::OnceLock;

    static SPECS: OnceLock<Result<Vec<OperationalDetailSpec>, MojoError>> = OnceLock::new();

    let detail = usize::try_from(detail)
        .ok()
        .filter(|detail| *detail < 62)
        .ok_or(MojoError::InvalidInput)?;
    match SPECS.get_or_init(|| (0_i64..62).map(load_operational_detail_spec).collect()) {
        Ok(specs) => specs.get(detail).ok_or(MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn operational_event_detail_plan(
    source: &str,
    first_local_chunk: bool,
) -> Result<Vec<i64>, MojoError> {
    const MAX_DETAILS: usize = 64;
    if source.is_empty() {
        return Err(MojoError::InvalidInput);
    }
    let source_length = i64::try_from(source.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [0_i64; MAX_DETAILS];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_mojo_operational_event_detail_plan_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            source.as_ptr() as usize as u64,
            source_length,
            i64::from(first_local_chunk),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output_count as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > output.len()
        || output[..output_count]
            .iter()
            .any(|value| !(0..=61).contains(value))
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(output[..output_count].to_vec())
}

pub fn label(kind: i64, value: i64) -> Result<String, MojoError> {
    if kind < 0 || value < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut output_length = 0_i64;
    let status = unsafe {
        prodex_mojo_observability_label_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            kind,
            value,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            (&mut output_length as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let length = usize::try_from(output_length).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..length].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

fn cached_fixed_label(
    kind: i64,
    value: i64,
    value_count: usize,
    cache: &'static std::sync::OnceLock<Result<Vec<String>, MojoError>>,
) -> Result<&'static str, MojoError> {
    let value = usize::try_from(value)
        .ok()
        .filter(|value| *value < value_count)
        .ok_or(MojoError::InvalidInput)?;
    match cache.get_or_init(|| {
        (0..value_count)
            .map(|value| {
                let value = i64::try_from(value).map_err(|_| MojoError::InvalidInput)?;
                label(kind, value)
            })
            .collect()
    }) {
        Ok(labels) => labels
            .get(value)
            .map(String::as_str)
            .ok_or(MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn runtime_websocket_local_pressure_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(147, value, 3, &LABELS)
}

pub fn runtime_websocket_task_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(148, value, 2, &LABELS)
}

pub fn runtime_websocket_worker_thread_prefix(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(149, value, 2, &LABELS)
}

pub fn runtime_websocket_dispatcher_thread_name(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(150, value, 2, &LABELS)
}

pub fn runtime_websocket_overflow_enqueue_event(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(151, value, 2, &LABELS)
}

pub fn runtime_websocket_overflow_dispatch_event(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(152, value, 2, &LABELS)
}

pub fn runtime_websocket_overflow_reject_event(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(153, value, 2, &LABELS)
}

pub fn runtime_websocket_direct_fallback_reason_label(
    value: i64,
) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(154, value, 2, &LABELS)
}

pub fn runtime_http_error_class_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(155, value, 6, &LABELS)
}

pub fn runtime_http_error_action_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(156, value, 3, &LABELS)
}

pub fn runtime_precommit_quota_block_reason_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(157, value, 3, &LABELS)
}

pub fn runtime_quota_pressure_band_reason_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(158, value, 5, &LABELS)
}

pub fn runtime_quota_window_status_reason_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(159, value, 5, &LABELS)
}

pub fn runtime_quota_source_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(160, value, 2, &LABELS)
}

pub fn runtime_previous_response_fallback_shape_label(
    value: i64,
) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(161, value, 5, &LABELS)
}

pub fn runtime_previous_response_retry_reason_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(162, value, 2, &LABELS)
}

pub fn runtime_previous_response_chain_reason_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(163, value, 2, &LABELS)
}

pub fn runtime_previous_response_outcome_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(164, value, 2, &LABELS)
}

pub fn runtime_soft_affinity_policy_reason_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(165, value, 8, &LABELS)
}

pub fn runtime_affinity_unavailable_reason_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(166, value, 7, &LABELS)
}

pub fn runtime_affinity_selection_kind_label(value: i64) -> Result<&'static str, MojoError> {
    static LABELS: std::sync::OnceLock<Result<Vec<String>, MojoError>> = std::sync::OnceLock::new();
    cached_fixed_label(167, value, 4, &LABELS)
}

/// Applies bounded privacy checks to borrowed metric key and value strings.
///
/// The Mojo kernel only reads the strings and returns a validation tag. It
/// never copies or returns their contents.
pub fn validate_telemetry_metric_label(
    key: &str,
    value: &str,
) -> Result<TelemetryMetricLabelValidation, MojoError> {
    let key_length = i64::try_from(key.len()).map_err(|_| MojoError::InvalidInput)?;
    let value_length = i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output_tag = -1_i64;
    let status = unsafe {
        prodex_mojo_observability_metric_label_validate_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            key.as_ptr() as usize as u64,
            key_length,
            value.as_ptr() as usize as u64,
            value_length,
            (&mut output_tag as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output_tag {
        0 => Ok(TelemetryMetricLabelValidation::Valid),
        1 => Ok(TelemetryMetricLabelValidation::InvalidKey),
        2 => Ok(TelemetryMetricLabelValidation::InvalidValue),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn metric_name(plan: i64, slot: i64) -> Result<String, MojoError> {
    if plan < 0 || slot < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut output_length = 0_i64;
    let status = unsafe {
        prodex_mojo_observability_metric_name_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            plan,
            slot,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            (&mut output_length as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let length = usize::try_from(output_length).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..length].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn label_key(key: i64) -> Result<String, MojoError> {
    if key < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_u8; OBSERVABILITY_LABEL_MAX_BYTES];
    let mut output_length = 0_i64;
    let status = unsafe {
        prodex_mojo_observability_label_key_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            key,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            (&mut output_length as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let length = usize::try_from(output_length).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..length].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservabilityPlanLabelSpec {
    pub key: i64,
    pub kind: i64,
}

unsafe extern "C" {
    fn prodex_mojo_observability_plan_label_spec_v1(
        abi_version: i64,
        plan: i64,
        slot: i64,
        key: u64,
        kind: u64,
    ) -> i64;
}

pub fn plan_label_spec(plan: i64, slot: i64) -> Result<ObservabilityPlanLabelSpec, MojoError> {
    if plan < 0 || slot < 0 {
        return Err(MojoError::InvalidInput);
    }
    let mut key = -1_i64;
    let mut kind = -1_i64;
    let status = unsafe {
        prodex_mojo_observability_plan_label_spec_v1(
            OBSERVABILITY_LABEL_ABI_VERSION,
            plan,
            slot,
            (&mut key as *mut i64) as u64,
            (&mut kind as *mut i64) as u64,
        )
    };
    match status {
        0 if key >= 0 && kind >= 0 => Ok(ObservabilityPlanLabelSpec { key, kind }),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_are_bounded_and_reject_unknowns() {
        assert!(label(0, 0).is_ok());
        assert_eq!(label(-1, 0), Err(MojoError::InvalidInput));
        assert_eq!(label(0, -1), Err(MojoError::InvalidInput));
        assert!(label(10_000, 0).is_err());
    }

    #[test]
    fn websocket_executor_labels_are_mojo_owned() {
        assert_eq!(
            runtime_websocket_local_pressure_label(0).unwrap(),
            "dns_resolve_timeout"
        );
        assert_eq!(
            runtime_websocket_local_pressure_label(2).unwrap(),
            "tcp_connect_executor_overflow"
        );
        assert_eq!(runtime_websocket_task_label(0).unwrap(), "tcp_connect");
        assert_eq!(
            runtime_websocket_worker_thread_prefix(1).unwrap(),
            "prodex-ws-dns"
        );
        assert_eq!(
            runtime_websocket_dispatcher_thread_name(0).unwrap(),
            "prodex-ws-connect-dispatch"
        );
        assert_eq!(
            runtime_websocket_overflow_enqueue_event(1).unwrap(),
            "websocket_dns_overflow_enqueue"
        );
        assert_eq!(
            runtime_websocket_overflow_dispatch_event(0).unwrap(),
            "websocket_connect_overflow_dispatch"
        );
        assert_eq!(
            runtime_websocket_overflow_reject_event(1).unwrap(),
            "websocket_dns_overflow_reject"
        );
        assert_eq!(
            runtime_websocket_task_label(2),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn runtime_proxy_observability_labels_are_mojo_owned() {
        assert_eq!(
            runtime_websocket_direct_fallback_reason_label(0).unwrap(),
            "precommit_budget_exhausted"
        );
        assert_eq!(runtime_http_error_class_label(4).unwrap(), "transient_5xx");
        assert_eq!(
            runtime_http_error_action_label(1).unwrap(),
            "rotate_profile"
        );
        assert_eq!(
            runtime_precommit_quota_block_reason_label(2).unwrap(),
            "quota_windows_unavailable_after_reprobe"
        );
        assert_eq!(
            runtime_quota_pressure_band_reason_label(3).unwrap(),
            "quota_exhausted"
        );
        assert_eq!(
            runtime_quota_window_status_reason_label(2).unwrap(),
            "critical"
        );
        assert_eq!(runtime_quota_source_label(1).unwrap(), "persisted_snapshot");
        assert_eq!(
            runtime_http_error_class_label(6),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(runtime_quota_source_label(2), Err(MojoError::InvalidInput));
    }

    #[test]
    fn operational_event_source_labels_are_mojo_owned() {
        assert_eq!(operational_event_source_label(0).unwrap(), None);
        assert_eq!(operational_event_source_label(1).unwrap(), Some("request"));
        assert_eq!(operational_event_source_label(2).unwrap(), Some("mcp"));
        assert_eq!(operational_event_source_label(19).unwrap(), Some("event"));
        assert_eq!(
            operational_event_source_label(20),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn operational_detail_specs_are_mojo_owned() {
        let specs = (0..62)
            .map(operational_detail_spec)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(specs.len(), 62);
        assert_eq!(specs[0].key, "profile");
        assert_eq!(specs[0].label, "profile");
        assert_eq!(specs[0].format, OperationalDetailFormat::Plain);
        assert_eq!(specs[35].key, "five_hour_remaining");
        assert_eq!(specs[35].label, "5h");
        assert_eq!(specs[35].format, OperationalDetailFormat::Percent);
        assert_eq!(specs[15].key, "path");
        assert_eq!(specs[15].label, "path");
        assert_eq!(specs[15].format, OperationalDetailFormat::Endpoint);
        assert_eq!(operational_detail_spec(62), Err(MojoError::InvalidInput));
    }

    #[test]
    fn plan_label_metadata_is_bounded_and_stable() {
        assert_eq!(
            plan_label_spec(68, 0),
            Ok(ObservabilityPlanLabelSpec {
                key: 132,
                kind: 124
            })
        );
        assert_eq!(
            plan_label_spec(63, 4),
            Ok(ObservabilityPlanLabelSpec { key: 82, kind: 118 })
        );
        assert_eq!(
            plan_label_spec(76, 0),
            Ok(ObservabilityPlanLabelSpec { key: 94, kind: 64 })
        );
        assert!(plan_label_spec(68, 9).is_err());
        assert!(plan_label_spec(10_000, 0).is_err());
    }
}
