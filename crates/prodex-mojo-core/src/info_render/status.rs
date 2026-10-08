use super::{
    ABI_VERSION, InfoStringView, MojoError, prodex_terminal_info_render_v1, render, render_bytes,
};

const STATUS_RUNTIME_PROFILE: i64 = 22;
const STATUS_RUNWAY: i64 = 23;
const STATUS_FIELDS: i64 = 24;
const STATUS_RESOURCE_METRICS: i64 = 25;
const STATUS_RESOURCE_HISTORY: i64 = 26;
const STATUS_QUOTA_GAUGE: i64 = 27;
pub(super) const STATUS_PROFILE_FILTER: i64 = 20;
pub(super) const STATUS_TOKEN_PLAN: i64 = 21;
pub(super) const STATUS_HISTORY_LIMIT: usize = 64;
const STATUS_FIELD_COUNT: usize = 14;
const STATUS_FIELDS_FIXED_INPUTS: usize = 18;

/// One parsed status token event passed to Mojo for ordering and history selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoStatusTokenEvent<'a> {
    pub timestamp: &'a str,
    pub request: Option<u64>,
    pub profile: &'a str,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Mojo-selected first/latest status events and bounded chronological token history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoStatusTokenPlan {
    pub first_index: Option<usize>,
    pub latest_index: Option<usize>,
    pub history: Vec<u64>,
}

/// Runtime profile candidate used by the status precedence policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoStatusRuntimeCandidate<'a> {
    pub timestamp: i64,
    pub profile: &'a str,
}

/// Selected source for the profile shown by `prodex status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoStatusProfileChoice {
    Active,
    Token,
    Runtime(usize),
}

/// Precise reset timestamp and host-formatted reset label for status runway output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoStatusReset<'a> {
    pub timestamp: i64,
    pub text: &'a str,
}

/// Status runway estimate supplied after host log and clock acquisition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoStatusRunwayEstimate<'a> {
    pub burn_per_hour: f64,
    pub observed_profiles: usize,
    pub observed_span_seconds: i64,
    pub exhaust_at: i64,
    pub exhaust_text: &'a str,
}

/// Typed inputs for the `prodex status` runway formatter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoStatusRunway<'a> {
    pub profiles_with_data: usize,
    pub current_remaining: i64,
    pub earliest_reset: Option<InfoStatusReset<'a>>,
    pub estimate: Option<InfoStatusRunwayEstimate<'a>>,
    pub now: i64,
}

/// Raw resource snapshot displayed by the status summary.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InfoStatusResources {
    pub available: bool,
    pub process_count: usize,
    pub runtime_process_count: usize,
    pub cpu_percent: Option<f64>,
    pub resident_bytes: u64,
    pub memory_total_bytes: u64,
    pub socket_count: usize,
    pub network_rx_queue_bytes: u64,
    pub network_tx_queue_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub disk_read_bytes_per_second: u64,
    pub disk_write_bytes_per_second: u64,
}

/// Host counters needed to derive status CPU and disk rates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InfoStatusResourceCounters {
    pub available: bool,
    pub process_cpu_ticks: u64,
    pub system_cpu_ticks: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

/// Mojo-derived rates for a status resource snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InfoStatusResourceMetrics {
    pub cpu_percent: Option<f64>,
    pub disk_read_bytes_per_second: u64,
    pub disk_write_bytes_per_second: u64,
}

/// Mojo-owned quota gauge label, ratio, and color band for Ratatui mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct InfoStatusQuotaGauge {
    pub ratio: f64,
    pub band: u8,
    pub label: String,
}

/// Typed inputs for the fixed human-facing status field list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoStatusFields<'a> {
    pub runtime_profile: &'a str,
    pub active_profile: &'a str,
    pub profile_count: usize,
    pub quota_compatible_profiles: usize,
    pub unavailable_profiles: usize,
    pub five_hour_quota: &'a str,
    pub five_hour_runway: &'a str,
    pub weekly_quota: &'a str,
    pub weekly_runway: &'a str,
    pub token_usage_summary: &'a str,
    pub token_input: u64,
    pub token_cached_input: u64,
    pub token_output: u64,
    pub token_history_text: &'a str,
    pub token_first_at: Option<&'a str>,
    pub token_last_at: Option<&'a str>,
    pub resources: InfoStatusResources,
    pub recent_load: &'a str,
    pub updated_at: &'a str,
}

fn render_numeric_fields(
    operation: i64,
    unsigned: &[u64],
    presence: u64,
    output: &mut [u64],
) -> Result<(), MojoError> {
    let output_capacity = output
        .len()
        .checked_mul(std::mem::size_of::<u64>())
        .ok_or(MojoError::InvalidInput)?;
    let mut written = -1_i64;
    let status = unsafe {
        prodex_terminal_info_render_v1(
            ABI_VERSION,
            operation,
            std::ptr::null::<i64>() as usize as u64,
            0,
            unsigned.as_ptr() as usize as u64,
            i64::try_from(unsigned.len()).map_err(|_| MojoError::InvalidInput)?,
            std::ptr::null::<InfoStringView>() as usize as u64,
            0,
            presence,
            output.as_mut_ptr().cast::<u8>() as usize as u64,
            i64::try_from(output_capacity).map_err(|_| MojoError::InvalidInput)?,
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
    if usize::try_from(written).map_err(|_| MojoError::InvalidOutput)? != output_capacity {
        return Err(MojoError::InvalidOutput);
    }
    Ok(())
}

/// Derive status CPU and disk rates from host counters in Mojo.
pub fn status_resource_metrics(
    previous: Option<(InfoStatusResourceCounters, f64)>,
    current: InfoStatusResourceCounters,
) -> Result<InfoStatusResourceMetrics, MojoError> {
    let (previous_counters, elapsed_seconds) =
        previous.map_or((None, 0.0), |(previous, elapsed)| (Some(previous), elapsed));
    let previous = previous_counters.unwrap_or_default();
    let unsigned = [
        u64::from(previous_counters.is_some()),
        u64::from(
            previous_counters
                .map(|value| value.available)
                .unwrap_or_default(),
        ),
        u64::from(current.available),
        previous.process_cpu_ticks,
        previous.system_cpu_ticks,
        previous.disk_read_bytes,
        previous.disk_write_bytes,
        current.process_cpu_ticks,
        current.system_cpu_ticks,
        current.disk_read_bytes,
        current.disk_write_bytes,
        elapsed_seconds.to_bits(),
    ];
    let mut output = [0_u64; 4];
    render_numeric_fields(STATUS_RESOURCE_METRICS, &unsigned, 0, &mut output)?;
    if output[0] > 1 || (output[0] == 0 && output[1] != 0) {
        return Err(MojoError::InvalidOutput);
    }
    let cpu_percent = (output[0] == 1).then(|| f64::from_bits(output[1]));
    if cpu_percent.is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value)) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(InfoStatusResourceMetrics {
        cpu_percent,
        disk_read_bytes_per_second: output[2],
        disk_write_bytes_per_second: output[3],
    })
}

/// Reduce status resource samples to the four chart points in Mojo.
pub fn status_resource_history_values(
    cpu_percent: Option<f64>,
    resident_bytes: u64,
    disk_bytes_per_second: (u64, u64),
    network_queue_bytes: (u64, u64),
) -> Result<[u64; 4], MojoError> {
    let mut output = [0_u64; 4];
    render_numeric_fields(
        STATUS_RESOURCE_HISTORY,
        &[
            cpu_percent.unwrap_or_default().to_bits(),
            resident_bytes,
            disk_bytes_per_second.0,
            disk_bytes_per_second.1,
            network_queue_bytes.0,
            network_queue_bytes.1,
        ],
        u64::from(cpu_percent.is_some()),
        &mut output,
    )?;
    Ok(output)
}

/// Format one status quota gauge while leaving terminal color mapping to Rust.
pub fn format_status_quota_gauge(
    total_remaining: i64,
    profiles: usize,
    earliest_reset_at: Option<i64>,
    now: i64,
    absolute_reset_text: Option<&str>,
) -> Result<InfoStatusQuotaGauge, MojoError> {
    let output = render(
        STATUS_QUOTA_GAUGE,
        &[total_remaining, earliest_reset_at.unwrap_or_default(), now],
        &[u64::try_from(profiles).map_err(|_| MojoError::InvalidInput)?],
        &[absolute_reset_text.unwrap_or_default()],
        u64::from(earliest_reset_at.is_some()),
    )?;
    let mut parts = output.splitn(3, ';');
    let band = parts
        .next()
        .ok_or(MojoError::InvalidOutput)?
        .parse::<u8>()
        .map_err(|_| MojoError::InvalidOutput)?;
    let ratio = parts
        .next()
        .ok_or(MojoError::InvalidOutput)?
        .parse::<u64>()
        .map_err(|_| MojoError::InvalidOutput)?;
    let label = parts.next().ok_or(MojoError::InvalidOutput)?.to_string();
    if band > 2 || ratio > 100 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(InfoStatusQuotaGauge {
        ratio: ratio as f64 / 100.0,
        band,
        label,
    })
}

/// Return eligible status profile positions in their input order.
pub fn status_profile_indices(eligible: &[bool]) -> Result<Vec<usize>, MojoError> {
    let output = render(
        STATUS_PROFILE_FILTER,
        &[],
        &eligible
            .iter()
            .map(|value| u64::from(*value))
            .collect::<Vec<_>>(),
        &[],
        0,
    )?;
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let mut previous = None;
    output
        .split(',')
        .map(|value| {
            let index = value
                .parse::<usize>()
                .map_err(|_| MojoError::InvalidOutput)?;
            if index >= eligible.len()
                || !eligible[index]
                || previous.is_some_and(|previous| index <= previous)
            {
                return Err(MojoError::InvalidOutput);
            }
            previous = Some(index);
            Ok(index)
        })
        .collect()
}

/// Select the chronological token history and first/latest event in Mojo.
pub fn status_token_plan(
    events: &[InfoStatusTokenEvent<'_>],
) -> Result<InfoStatusTokenPlan, MojoError> {
    let mut unsigned = Vec::with_capacity(events.len().saturating_mul(4));
    let mut texts = Vec::with_capacity(events.len().saturating_mul(2));
    for event in events {
        unsigned.extend([
            u64::from(event.request.is_some()),
            event.request.unwrap_or_default(),
            event.input_tokens,
            event.output_tokens,
        ]);
        texts.extend([event.timestamp, event.profile]);
    }
    let output = render(STATUS_TOKEN_PLAN, &[], &unsigned, &texts, 0)?;
    parse_status_token_plan(&output, events.len())
}

/// Resolve the profile precedence used by status from active, token, and runtime inputs.
pub fn status_profile_choice(
    runtime_process_count: usize,
    active_profile: &str,
    token_profile: Option<&str>,
    runtime_candidates: &[InfoStatusRuntimeCandidate<'_>],
) -> Result<InfoStatusProfileChoice, MojoError> {
    let mut texts = Vec::with_capacity(runtime_candidates.len().saturating_add(2));
    texts.extend([active_profile, token_profile.unwrap_or_default()]);
    texts.extend(runtime_candidates.iter().map(|candidate| candidate.profile));
    let signed = runtime_candidates
        .iter()
        .map(|candidate| candidate.timestamp)
        .collect::<Vec<_>>();
    let output = render(
        STATUS_RUNTIME_PROFILE,
        &signed,
        &[u64::from(runtime_process_count > 0)],
        &texts,
        u64::from(token_profile.is_some()),
    )?;
    let selected = output
        .parse::<usize>()
        .map_err(|_| MojoError::InvalidOutput)?;
    match selected {
        0 => Ok(InfoStatusProfileChoice::Active),
        1 if token_profile.is_some() => Ok(InfoStatusProfileChoice::Token),
        index if index >= 2 && index - 2 < runtime_candidates.len() => {
            Ok(InfoStatusProfileChoice::Runtime(index - 2))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Format the quota runway text shown by the status command.
pub fn format_status_runway(input: InfoStatusRunway<'_>) -> Result<String, MojoError> {
    let (reset_at, reset_text) = input
        .earliest_reset
        .map_or((0, ""), |reset| (reset.timestamp, reset.text));
    let (exhaust_at, observed_span, observed_profiles, burn, exhaust_text) =
        input.estimate.map_or((0, 0, 0, 0.0, ""), |estimate| {
            (
                estimate.exhaust_at,
                estimate.observed_span_seconds,
                estimate.observed_profiles,
                estimate.burn_per_hour,
                estimate.exhaust_text,
            )
        });
    let burn_fields = [
        u64::try_from(input.profiles_with_data).map_err(|_| MojoError::InvalidInput)?,
        u64::try_from(observed_profiles).map_err(|_| MojoError::InvalidInput)?,
        burn.to_bits(),
    ];
    render(
        STATUS_RUNWAY,
        &[
            input.current_remaining,
            reset_at,
            exhaust_at,
            observed_span,
            input.now,
        ],
        &burn_fields,
        &[reset_text, exhaust_text],
        u64::from(input.earliest_reset.is_some()) | (u64::from(input.estimate.is_some()) << 1),
    )
}

fn parse_status_token_plan(
    output: &str,
    event_count: usize,
) -> Result<InfoStatusTokenPlan, MojoError> {
    let mut parts = output.splitn(4, ';');
    let first_index = parse_status_index(parts.next().ok_or(MojoError::InvalidOutput)?)?;
    let latest_index = parse_status_index(parts.next().ok_or(MojoError::InvalidOutput)?)?;
    let history_count = parts
        .next()
        .ok_or(MojoError::InvalidOutput)?
        .parse::<usize>()
        .map_err(|_| MojoError::InvalidOutput)?;
    let history_text = parts.next().ok_or(MojoError::InvalidOutput)?;
    if first_index.is_some_and(|index| index >= event_count)
        || latest_index.is_some_and(|index| index >= event_count)
        || first_index.is_some_and(|first| latest_index.is_some_and(|last| first > last))
        || history_count > event_count.min(STATUS_HISTORY_LIMIT)
        || (event_count == 0) != first_index.is_none()
        || first_index.is_none() != latest_index.is_none()
    {
        return Err(MojoError::InvalidOutput);
    }
    let history = if history_count == 0 {
        if !history_text.is_empty() {
            return Err(MojoError::InvalidOutput);
        }
        Vec::new()
    } else {
        let values = history_text
            .split(',')
            .map(|value| value.parse::<u64>().map_err(|_| MojoError::InvalidOutput))
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() != history_count {
            return Err(MojoError::InvalidOutput);
        }
        values
    };
    Ok(InfoStatusTokenPlan {
        first_index,
        latest_index,
        history,
    })
}

fn parse_status_index(value: &str) -> Result<Option<usize>, MojoError> {
    if value == "-" {
        Ok(None)
    } else {
        value
            .parse::<usize>()
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput)
    }
}

fn parse_status_fields(output: &[u8]) -> Result<Vec<(String, String)>, MojoError> {
    let mut cursor = 0;
    let mut fields = Vec::with_capacity(STATUS_FIELD_COUNT);
    while cursor < output.len() {
        if fields.len() == STATUS_FIELD_COUNT {
            return Err(MojoError::InvalidOutput);
        }
        let label_len = read_status_length(output, &mut cursor, None)?;
        let label_end = cursor
            .checked_add(label_len)
            .ok_or(MojoError::InvalidOutput)?;
        let label = std::str::from_utf8(
            output
                .get(cursor..label_end)
                .ok_or(MojoError::InvalidOutput)?,
        )
        .map_err(|_| MojoError::InvalidOutput)?
        .to_string();
        cursor = label_end;

        let value_len = read_status_length(output, &mut cursor, Some(20))?;
        let value_end = cursor
            .checked_add(value_len)
            .ok_or(MojoError::InvalidOutput)?;
        let value = std::str::from_utf8(
            output
                .get(cursor..value_end)
                .ok_or(MojoError::InvalidOutput)?,
        )
        .map_err(|_| MojoError::InvalidOutput)?
        .to_string();
        cursor = value_end;
        fields.push((label, value));
    }
    if fields.len() != STATUS_FIELD_COUNT {
        return Err(MojoError::InvalidOutput);
    }
    Ok(fields)
}

fn read_status_length(
    bytes: &[u8],
    cursor: &mut usize,
    fixed_width: Option<usize>,
) -> Result<usize, MojoError> {
    let start = *cursor;
    let mut value = 0_usize;
    while let Some(byte @ b'0'..=b'9') = bytes.get(*cursor).copied() {
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(usize::from(byte - b'0')))
            .ok_or(MojoError::InvalidOutput)?;
        *cursor += 1;
    }
    let width = cursor.checked_sub(start).ok_or(MojoError::InvalidOutput)?;
    if width == 0
        || fixed_width.is_some_and(|expected| width != expected)
        || bytes.get(*cursor) != Some(&b':')
    {
        return Err(MojoError::InvalidOutput);
    }
    *cursor += 1;
    Ok(value)
}

/// Format status field labels and values in their established order in Mojo.
pub fn format_status_fields(
    input: InfoStatusFields<'_>,
) -> Result<Vec<(String, String)>, MojoError> {
    let resources = input.resources;
    let cpu_percent = resources.cpu_percent.unwrap_or_default();
    let mut unsigned = Vec::with_capacity(STATUS_FIELDS_FIXED_INPUTS);
    unsigned.extend([
        u64::try_from(input.profile_count).map_err(|_| MojoError::InvalidInput)?,
        u64::try_from(input.quota_compatible_profiles).map_err(|_| MojoError::InvalidInput)?,
        u64::try_from(input.unavailable_profiles).map_err(|_| MojoError::InvalidInput)?,
        input.token_input,
        input.token_cached_input,
        input.token_output,
        resources.resident_bytes,
        resources.memory_total_bytes,
        u64::try_from(resources.process_count).map_err(|_| MojoError::InvalidInput)?,
        u64::try_from(resources.runtime_process_count).map_err(|_| MojoError::InvalidInput)?,
        u64::try_from(resources.socket_count).map_err(|_| MojoError::InvalidInput)?,
        resources.network_rx_queue_bytes,
        resources.network_tx_queue_bytes,
        resources.disk_read_bytes,
        resources.disk_write_bytes,
        resources.disk_read_bytes_per_second,
        resources.disk_write_bytes_per_second,
        cpu_percent.to_bits(),
    ]);
    let texts = [
        input.runtime_profile,
        input.active_profile,
        input.five_hour_quota,
        input.five_hour_runway,
        input.weekly_quota,
        input.weekly_runway,
        input.token_usage_summary,
        input.token_first_at.unwrap_or_default(),
        input.token_last_at.unwrap_or_default(),
        input.recent_load,
        input.updated_at,
        input.token_history_text,
    ];
    let presence = u64::from(resources.available)
        | (u64::from(resources.cpu_percent.is_some()) << 1)
        | (u64::from(input.token_first_at.is_some()) << 2)
        | (u64::from(input.token_last_at.is_some()) << 3);
    parse_status_fields(&render_bytes(
        STATUS_FIELDS,
        &[],
        &unsigned,
        &texts,
        presence,
    )?)
}
