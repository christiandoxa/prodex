use crate::MojoError;

const ABI_VERSION: i64 = 1;
const METRICS_PER_ROW: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditUsageRowInput {
    pub recorded_at_epoch: i64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub cost_micros: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuditUsageSummary {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub cost_micros: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuditBudgetFlags {
    pub request_limit_reached: bool,
    pub token_limit_reached: bool,
    pub cost_limit_reached: bool,
}

unsafe extern "C" {
    fn prodex_audit_usage_token_normalize_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        fallback_address: u64,
        fallback_length: i64,
        max_chars: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_audit_usage_total_v1(
        abi_version: i64,
        current_total: u64,
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_audit_usage_summary_v1(
        abi_version: i64,
        epochs_address: u64,
        metrics_address: u64,
        row_count: i64,
        since_epoch: i64,
        until_epoch: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_audit_budget_flags_v1(
        abi_version: i64,
        requests: u64,
        total_tokens: u64,
        cost_micros: u64,
        max_requests_present: i64,
        max_requests: u64,
        max_tokens_present: i64,
        max_tokens: u64,
        max_cost_present: i64,
        max_cost_micros: u64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn normalize_usage_token(
    value: &str,
    fallback: &str,
    max_chars: usize,
) -> Result<String, MojoError> {
    let capacity = max_chars.max(fallback.len());
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    status(unsafe {
        prodex_audit_usage_token_normalize_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            fallback.as_ptr() as usize as u64,
            i64::try_from(fallback.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_chars).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn normalized_total_tokens(
    current_total: u64,
    input_tokens: u64,
    output_tokens: u64,
    reasoning_tokens: u64,
) -> Result<u64, MojoError> {
    let mut output = 0_u64;
    status(unsafe {
        prodex_audit_usage_total_v1(
            ABI_VERSION,
            current_total,
            input_tokens,
            output_tokens,
            reasoning_tokens,
            (&mut output as *mut u64) as usize as u64,
        )
    })?;
    Ok(output)
}

pub fn summarize_usage(
    rows: &[AuditUsageRowInput],
    since_epoch: i64,
    until_epoch: i64,
) -> Result<AuditUsageSummary, MojoError> {
    let mut epochs = Vec::with_capacity(rows.len());
    let mut metrics = Vec::with_capacity(rows.len().saturating_mul(METRICS_PER_ROW));
    for row in rows {
        epochs.push(row.recorded_at_epoch);
        metrics.extend_from_slice(&[
            row.input_tokens,
            row.output_tokens,
            row.cached_input_tokens,
            row.reasoning_tokens,
            row.total_tokens,
            row.cost_micros,
        ]);
    }
    let mut output = [0_u64; 7];
    status(unsafe {
        prodex_audit_usage_summary_v1(
            ABI_VERSION,
            epochs.as_ptr() as usize as u64,
            metrics.as_ptr() as usize as u64,
            i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?,
            since_epoch,
            until_epoch,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(AuditUsageSummary {
        requests: output[0],
        input_tokens: output[1],
        output_tokens: output[2],
        cached_input_tokens: output[3],
        reasoning_tokens: output[4],
        total_tokens: output[5],
        cost_micros: output[6],
    })
}

pub fn budget_flags(
    summary: AuditUsageSummary,
    max_requests: Option<u64>,
    max_tokens: Option<u64>,
    max_cost_micros: Option<u64>,
) -> Result<AuditBudgetFlags, MojoError> {
    let mut flags = 0_u64;
    status(unsafe {
        prodex_audit_budget_flags_v1(
            ABI_VERSION,
            summary.requests,
            summary.total_tokens,
            summary.cost_micros,
            i64::from(max_requests.is_some()),
            max_requests.unwrap_or_default(),
            i64::from(max_tokens.is_some()),
            max_tokens.unwrap_or_default(),
            i64::from(max_cost_micros.is_some()),
            max_cost_micros.unwrap_or_default(),
            (&mut flags as *mut u64) as usize as u64,
        )
    })?;
    if flags & !0b111 != 0 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(AuditBudgetFlags {
        request_limit_reached: flags & 1 != 0,
        token_limit_reached: flags & 2 != 0,
        cost_limit_reached: flags & 4 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_usage_policy_preserves_normalization_summary_and_limits() {
        assert_eq!(
            normalize_usage_token("  Project A β  ", "unknown", 64).unwrap(),
            "project-a"
        );
        assert_eq!(
            normalize_usage_token(" β ", "global", 64).unwrap(),
            "global"
        );
        assert_eq!(
            normalized_total_tokens(0, u64::MAX, 1, 7).unwrap(),
            u64::MAX
        );
        assert_eq!(normalized_total_tokens(9, 1, 2, 3).unwrap(), 9);

        let summary = summarize_usage(
            &[
                AuditUsageRowInput {
                    recorded_at_epoch: 10,
                    input_tokens: 2,
                    output_tokens: 3,
                    cached_input_tokens: 4,
                    reasoning_tokens: 5,
                    total_tokens: 10,
                    cost_micros: 7,
                },
                AuditUsageRowInput {
                    recorded_at_epoch: 20,
                    input_tokens: u64::MAX,
                    output_tokens: 1,
                    cached_input_tokens: 1,
                    reasoning_tokens: 1,
                    total_tokens: u64::MAX,
                    cost_micros: u64::MAX,
                },
            ],
            10,
            20,
        )
        .unwrap();
        assert_eq!(summary.requests, 2);
        assert_eq!(summary.input_tokens, u64::MAX);
        assert_eq!(summary.total_tokens, u64::MAX);
        assert_eq!(summary.cost_micros, u64::MAX);
        assert_eq!(
            budget_flags(summary, Some(2), Some(u64::MAX), None).unwrap(),
            AuditBudgetFlags {
                request_limit_reached: true,
                token_limit_reached: true,
                cost_limit_reached: false,
            }
        );
    }
}
