use crate::MojoError;

const ABI_VERSION: i64 = 1;
const RELATIVE_DURATION: i64 = 0;
const QUOTA_DATA: i64 = 1;
const RUNTIME_POLICY: i64 = 2;
const RUNTIME_LOGS: i64 = 3;
const TUNING_WORKERS: i64 = 4;
const TUNING_BUDGETS: i64 = 5;
const TUNING_TRANSPORT: i64 = 6;
const POOL_REMAINING: i64 = 7;
const PROCESS_SUMMARY: i64 = 8;
const LOAD_SUMMARY: i64 = 9;
const TOKEN_USAGE: i64 = 10;
const RUNTIME_LAUNCH_SELECTION: i64 = 11;
const RUNTIME_LAUNCH_WARNING: i64 = 12;
const RUNTIME_PROVIDER_DIRECT: i64 = 13;
const RUNTIME_QUOTA_HINT: i64 = 14;
const HUMAN_BYTES: i64 = 15;
const HUMAN_COUNT: i64 = 16;
const TOKEN_EFFICIENCY: i64 = 17;
const MEMORY_PERCENT: i64 = 18;
const TEXT_SPARKLINE: i64 = 19;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoTokenUsageProfile<'a> {
    pub profile: &'a str,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoRuntimeLaunchSelectedProfileStatus<'a> {
    Ready,
    Blocked { blocked_summary: &'a str },
    ProbeFailed { error: &'a str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoRuntimeLaunchScoredCandidateOutput {
    pub warning: Option<String>,
    pub selection: String,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct InfoStringView {
    ptr: u64,
    len: u64,
}

unsafe extern "C" {
    fn prodex_terminal_info_render_v1(
        abi_version: i64,
        operation: i64,
        signed_address: u64,
        signed_count: i64,
        unsigned_address: u64,
        unsigned_count: i64,
        text_address: u64,
        text_count: i64,
        presence: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn render(
    operation: i64,
    signed: &[i64],
    unsigned: &[u64],
    texts: &[&str],
    presence: u64,
) -> Result<String, MojoError> {
    let views = texts
        .iter()
        .map(|value| {
            Ok(InfoStringView {
                ptr: value.as_ptr() as usize as u64,
                len: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let text_bytes = texts.iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })?;
    let capacity = text_bytes
        .checked_add(4096)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_terminal_info_render_v1(
            ABI_VERSION,
            operation,
            signed.as_ptr() as usize as u64,
            i64::try_from(signed.len()).map_err(|_| MojoError::InvalidInput)?,
            unsigned.as_ptr() as usize as u64,
            i64::try_from(unsigned.len()).map_err(|_| MojoError::InvalidInput)?,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            presence,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
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
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn format_process_summary(
    total_count: usize,
    runtime_count: usize,
    max_visible_processes: usize,
    processes: &[String],
) -> Result<String, MojoError> {
    let texts = processes.iter().map(String::as_str).collect::<Vec<_>>();
    render(
        PROCESS_SUMMARY,
        &[],
        &[
            u64::try_from(total_count).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(runtime_count).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(max_visible_processes).map_err(|_| MojoError::InvalidInput)?,
        ],
        &texts,
        0,
    )
}

pub fn format_load_summary(
    log_count: usize,
    active_inflight_units: usize,
    recent_selection_events: usize,
    recent_first_timestamp: Option<i64>,
    recent_last_timestamp: Option<i64>,
    runtime_process_count: usize,
    recent_load_window_seconds: i64,
) -> Result<String, MojoError> {
    let timestamps_present = recent_first_timestamp.is_some() && recent_last_timestamp.is_some();
    render(
        LOAD_SUMMARY,
        &[
            recent_first_timestamp.unwrap_or_default(),
            recent_last_timestamp.unwrap_or_default(),
            recent_load_window_seconds,
        ],
        &[
            u64::try_from(log_count).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(active_inflight_units).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(recent_selection_events).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(runtime_process_count).map_err(|_| MojoError::InvalidInput)?,
        ],
        &[],
        u64::from(timestamps_present),
    )
}

pub fn format_token_usage_summary(
    event_count: usize,
    log_count: usize,
    total: [u64; 4],
    profiles: &[InfoTokenUsageProfile<'_>],
) -> Result<String, MojoError> {
    if profiles.len() > 4 {
        return Err(MojoError::InvalidInput);
    }
    let mut unsigned = Vec::with_capacity(6 + profiles.len() * 4);
    unsigned.extend_from_slice(&[
        u64::try_from(event_count).map_err(|_| MojoError::InvalidInput)?,
        u64::try_from(log_count).map_err(|_| MojoError::InvalidInput)?,
        total[0],
        total[1],
        total[2],
        total[3],
    ]);
    for profile in profiles {
        unsigned.extend_from_slice(&[
            profile.input_tokens,
            profile.cached_input_tokens,
            profile.output_tokens,
            profile.reasoning_tokens,
        ]);
    }
    let texts = profiles
        .iter()
        .map(|profile| profile.profile)
        .collect::<Vec<_>>();
    render(TOKEN_USAGE, &[], &unsigned, &texts, 0)
}

pub fn format_human_bytes(value: u64) -> Result<String, MojoError> {
    render(HUMAN_BYTES, &[], &[value], &[], 0)
}

pub fn format_human_count(value: u64) -> Result<String, MojoError> {
    render(HUMAN_COUNT, &[], &[value], &[], 0)
}

pub fn format_token_efficiency(input: u64, cached: u64, output: u64) -> Result<String, MojoError> {
    render(TOKEN_EFFICIENCY, &[], &[input, cached, output], &[], 0)
}

pub fn format_memory_percent(resident: u64, total: u64) -> Result<String, MojoError> {
    render(MEMORY_PERCENT, &[], &[resident, total], &[], 0)
}

pub fn format_text_sparkline(values: &[u64]) -> Result<String, MojoError> {
    render(TEXT_SPARKLINE, &[], values, &[], 0)
}

pub fn format_relative_duration(seconds: i64) -> Result<String, MojoError> {
    render(RELATIVE_DURATION, &[seconds], &[], &[], 0)
}

pub fn format_quota_data_summary(values: [u64; 4]) -> Result<String, MojoError> {
    render(QUOTA_DATA, &[], &values, &[], 0)
}

pub fn format_runtime_policy_summary(
    path: Option<&str>,
    version: Option<u32>,
) -> Result<String, MojoError> {
    let path = path.unwrap_or_default();
    let version_value = u64::from(version.unwrap_or_default());
    let presence = u64::from(!path.is_empty() && version.is_some()) * 3;
    render(RUNTIME_POLICY, &[], &[version_value], &[path], presence)
}

pub fn format_runtime_logs_summary(directory: &str, format: &str) -> Result<String, MojoError> {
    render(RUNTIME_LOGS, &[], &[], &[directory, format], 0)
}

pub fn format_runtime_tuning_workers(values: [u64; 16]) -> Result<String, MojoError> {
    render(TUNING_WORKERS, &[], &values, &[], 0)
}

pub fn format_runtime_tuning_budgets(values: [u64; 10]) -> Result<String, MojoError> {
    render(TUNING_BUDGETS, &[], &values, &[], 0)
}

pub fn format_runtime_tuning_transport(values: [u64; 9]) -> Result<String, MojoError> {
    render(TUNING_TRANSPORT, &[], &values, &[], 0)
}

pub fn format_pool_remaining(
    total_remaining: i64,
    profiles_with_data: usize,
    earliest_reset_text: Option<&str>,
) -> Result<String, MojoError> {
    let reset = earliest_reset_text.unwrap_or_default();
    render(
        POOL_REMAINING,
        &[total_remaining],
        &[u64::try_from(profiles_with_data).map_err(|_| MojoError::InvalidInput)?],
        &[reset],
        u64::from(earliest_reset_text.is_some()),
    )
}

pub fn format_runtime_launch_scored_candidate(
    initial_profile_name: &str,
    candidate_name: &str,
    quota_summary: &str,
    selected_profile_status: Option<InfoRuntimeLaunchSelectedProfileStatus<'_>>,
) -> Result<InfoRuntimeLaunchScoredCandidateOutput, MojoError> {
    let (status, detail) = match selected_profile_status {
        None => (0_u64, ""),
        Some(InfoRuntimeLaunchSelectedProfileStatus::Ready) => (1, ""),
        Some(InfoRuntimeLaunchSelectedProfileStatus::Blocked { blocked_summary }) => {
            (2, blocked_summary)
        }
        Some(InfoRuntimeLaunchSelectedProfileStatus::ProbeFailed { error }) => (3, error),
    };
    let texts = [initial_profile_name, candidate_name, quota_summary, detail];
    let selection = render(RUNTIME_LAUNCH_SELECTION, &[], &[], &texts, status)?;
    let warning = render(RUNTIME_LAUNCH_WARNING, &[], &[], &texts, status)?;
    Ok(InfoRuntimeLaunchScoredCandidateOutput {
        warning: (!warning.is_empty()).then_some(warning),
        selection,
    })
}

pub fn format_runtime_provider_direct_launch_message(
    provider_id: &str,
    source: &str,
) -> Result<String, MojoError> {
    render(RUNTIME_PROVIDER_DIRECT, &[], &[], &[provider_id, source], 0)
}

pub fn format_runtime_launch_quota_inspect_hint(profile_name: &str) -> Result<String, MojoError> {
    render(RUNTIME_QUOTA_HINT, &[], &[], &[profile_name], 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_render_preserves_scalar_summary_contracts() {
        assert_eq!(format_relative_duration(0).unwrap(), "now");
        assert_eq!(format_relative_duration(59).unwrap(), "<1m");
        assert_eq!(format_relative_duration(3_660).unwrap(), "1h 1m");
        assert_eq!(format_relative_duration(90_000).unwrap(), "1d 1h");

        assert_eq!(
            format_quota_data_summary([3, 1, 1, 1]).unwrap(),
            "3 quota-compatible profile(s): live=1, snapshot=1, unavailable=1"
        );
        assert_eq!(
            format_quota_data_summary([0, 0, 0, 0]).unwrap(),
            "No quota-compatible profiles"
        );
        assert_eq!(
            format_runtime_policy_summary(Some("/tmp/policy.json"), Some(7)).unwrap(),
            "/tmp/policy.json (v7)"
        );
        assert_eq!(
            format_runtime_policy_summary(Some("/tmp/policy.json"), None).unwrap(),
            "disabled"
        );
        assert_eq!(
            format_runtime_logs_summary("/tmp/logs", "json").unwrap(),
            "/tmp/logs (json)"
        );
        assert_eq!(
            format_pool_remaining(42, 2, Some("in 1h")).unwrap(),
            "42% across 2 profile(s); earliest reset in 1h"
        );
        assert_eq!(
            format_pool_remaining(42, 0, Some("in 1h")).unwrap(),
            "Unavailable"
        );
        assert_eq!(format_human_bytes(999).unwrap(), "999 B");
        assert_eq!(format_human_bytes(1536).unwrap(), "1.5 KiB");
        assert_eq!(format_human_count(999).unwrap(), "999");
        assert_eq!(format_human_count(1_500).unwrap(), "1.5K");
        assert_eq!(
            format_token_efficiency(200, 50, 100).unwrap(),
            "cache hit 25.0% · output share 33.3%"
        );
        assert_eq!(format_memory_percent(512, 1024).unwrap(), "50.0%");
        assert_eq!(format_text_sparkline(&[]).unwrap(), "-");
        assert_eq!(format_text_sparkline(&[0, 0]).unwrap(), "-");
        assert_eq!(
            format_text_sparkline(&[1, 2, 3]).unwrap().chars().count(),
            3
        );

        assert_eq!(
            format_runtime_tuning_workers([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
                .unwrap(),
            "workers proxy=1, long-lived=2, async=3, probe-refresh=4; active=5, queue=6; lanes responses=7, compact=8, websocket=9, standard=10; ws-connect workers=11, queue=12, overflow=13; ws-dns workers=14, queue=15, overflow=16"
        );
        assert_eq!(
            format_runtime_tuning_budgets([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).unwrap(),
            "precommit=1x/2ms, pressure-precommit=3x/4ms, continuation=5x/6ms; admission=7ms, pressure-admission=8ms, long-lived=9ms, pressure-long-lived=10ms"
        );
        assert_eq!(
            format_runtime_tuning_transport([1, 2, 3, 4, 5, 6, 7, 8, 9]).unwrap(),
            "http-connect=1ms, stream-idle=2ms, sse-lookahead=3ms; ws-connect=4ms, ws-progress=5ms, ws-happy=6ms, ws-stale-reuse=7ms; inflight soft/hard=8/9"
        );
        assert_eq!(
            format_process_summary(
                7,
                2,
                6,
                &["10/run", "11/run", "12/run", "13/run", "14/run", "15/run"]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
            "Yes (7 total, 2 runtime; processes: 10/run, 11/run, 12/run, 13/run, 14/run, 15/run (+1 more))"
        );
        assert_eq!(
            format_load_summary(2, 3, 4, Some(100), Some(3_760), 1, 1_800).unwrap(),
            "4 selection event(s) over 1h 1m; inflight units 3; 2 active runtime log(s)"
        );
        assert_eq!(
            format_token_usage_summary(
                2,
                3,
                [110, 25, 44, 9],
                &[InfoTokenUsageProfile {
                    profile: "main",
                    input_tokens: 100,
                    cached_input_tokens: 25,
                    output_tokens: 40,
                    reasoning_tokens: 8,
                }],
            )
            .unwrap(),
            "2 event(s), logs=3: input=110, cached_input=25, output=44, reasoning=9; by profile: main:100 in/25 cached/40 out/8 reasoning"
        );
    }
}
