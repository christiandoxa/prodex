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
    }
}
