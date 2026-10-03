use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThroughputSamplePlan {
    pub counter_reset: bool,
    pub append_sample: bool,
}

unsafe extern "C" {
    fn prodex_log_throughput_sample_plan_v1(
        abi_version: i64,
        previous_present: i64,
        previous_tokens: u64,
        previous_generation_ms: u64,
        current_tokens: u64,
        current_generation_ms: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_completed_rate_v1(
        abi_version: i64,
        output_tokens: u64,
        generation_ms: u64,
        valid_address: u64,
        rate_address: u64,
    ) -> i64;

    fn prodex_log_retention_policy_v1(
        abi_version: i64,
        operation: i64,
        input0: u64,
        input1: u64,
        input2: u64,
        input3: u64,
        input4: u64,
        input5: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_log_throughput_stream_rate_v1(
        abi_version: i64,
        first_tokens: u64,
        first_generation_ms: u64,
        last_tokens: u64,
        last_generation_ms: u64,
        valid_address: u64,
        rate_address: u64,
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

fn bool_output(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

const RETENTION_BOUNDED_VALUE: i64 = 1;
const RETENTION_ROTATION: i64 = 2;
const RETENTION_BOUNDED_TEXT: i64 = 5;
const RETENTION_EXPIRED_CANDIDATES: i64 = 6;
const RETENTION_OVER_BUDGET_CANDIDATES: i64 = 7;
const RETENTION_CANDIDATE_STRIDE: usize = 5;

fn retention_call(operation: i64, input: [u64; 6]) -> Result<[u64; 4], MojoError> {
    let mut output = [0_u64; 4];
    status(unsafe {
        prodex_log_retention_policy_v1(
            ABI_VERSION,
            operation,
            input[0],
            input[1],
            input[2],
            input[3],
            input[4],
            input[5],
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(output)
}

pub fn bounded_text_policy_value(
    value: Option<&str>,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, MojoError> {
    let (address, length, present) = match value {
        Some(value) => (
            value.as_ptr() as usize as u64,
            u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            1_u64,
        ),
        None => (0, 0, 0),
    };
    Ok(retention_call(
        RETENTION_BOUNDED_TEXT,
        [address, length, present, default, min, max],
    )?[0])
}

pub fn bounded_policy_value(
    value: Option<u64>,
    default: u64,
    min: u64,
    max: u64,
) -> Result<u64, MojoError> {
    Ok(retention_call(
        RETENTION_BOUNDED_VALUE,
        [
            u64::from(value.is_some()),
            value.unwrap_or_default(),
            default,
            min,
            max,
            0,
        ],
    )?[0])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogRotationPlan {
    pub rotate_before_write: bool,
    pub rotate_after_oversized_line: bool,
}

pub fn log_rotation_plan(
    current_size: u64,
    line_len: u64,
    max_file_bytes: u64,
) -> Result<LogRotationPlan, MojoError> {
    let output = retention_call(
        RETENTION_ROTATION,
        [current_size, line_len, max_file_bytes, 0, 0, 0],
    )?;
    Ok(LogRotationPlan {
        rotate_before_write: bool_output(
            i64::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        )?,
        rotate_after_oversized_line: bool_output(
            i64::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        )?,
    })
}

fn signed_order_key(value: i64) -> u64 {
    (value as u64) ^ (1_u64 << 63)
}

/// Host-observed runtime-log facts used by retention planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogRetentionCandidate<'a> {
    /// Encoded basename bytes used for the path tie-break within one directory.
    pub file_name: &'a [u8],
    /// Current file size in bytes.
    pub size: u64,
    /// Filesystem modification time as Unix epoch seconds.
    pub modified_epoch_seconds: i64,
    /// Whether host-side locks and retention protections allow deletion.
    pub removable: bool,
    /// Excluded because it was removed already or a deletion attempt failed.
    pub unavailable: bool,
}

#[derive(Debug, Clone, Copy)]
enum LogRetentionCandidatePolicy {
    Expired {
        oldest_allowed_epoch_seconds: i64,
    },
    OverBudget {
        remaining_count: usize,
        max_files: usize,
        total_bytes: u64,
        total_budget: u64,
    },
}

unsafe extern "C" {
    fn prodex_log_retention_candidates_v1(
        abi_version: i64,
        operation: i64,
        candidate_count: u64,
        candidates_address: u64,
        file_names_address: u64,
        file_names_length: u64,
        policy_address: u64,
        output_address: u64,
    ) -> i64;
}

fn retention_candidates_call(
    candidates: &[LogRetentionCandidate<'_>],
    policy: LogRetentionCandidatePolicy,
) -> Result<Vec<u64>, MojoError> {
    let (operation, policy_values) = match policy {
        LogRetentionCandidatePolicy::Expired {
            oldest_allowed_epoch_seconds,
        } => (
            RETENTION_EXPIRED_CANDIDATES,
            [signed_order_key(oldest_allowed_epoch_seconds), 0, 0, 0, 0],
        ),
        LogRetentionCandidatePolicy::OverBudget {
            remaining_count,
            max_files,
            total_bytes,
            total_budget,
        } => (
            RETENTION_OVER_BUDGET_CANDIDATES,
            [
                0,
                u64::try_from(remaining_count).map_err(|_| MojoError::InvalidInput)?,
                u64::try_from(max_files).map_err(|_| MojoError::InvalidInput)?,
                total_bytes,
                total_budget,
            ],
        ),
    };
    let (candidate_rows, file_names) = encode_retention_candidates(candidates, policy)?;
    retention_candidates_abi_call(operation, &candidate_rows, &file_names, policy_values)
}

fn retention_candidates_abi_call(
    operation: i64,
    candidate_rows: &[u64],
    file_names: &[u8],
    policy: [u64; 5],
) -> Result<Vec<u64>, MojoError> {
    if !candidate_rows
        .len()
        .is_multiple_of(RETENTION_CANDIDATE_STRIDE)
    {
        return Err(MojoError::InvalidInput);
    }
    let candidate_count = candidate_rows.len() / RETENTION_CANDIDATE_STRIDE;
    let candidate_count_u64 =
        u64::try_from(candidate_count).map_err(|_| MojoError::InvalidInput)?;
    let output_len = candidate_count
        .checked_mul(2)
        .ok_or(MojoError::InvalidInput)?;
    let file_names_length = u64::try_from(file_names.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = vec![0_u64; output_len];
    status(unsafe {
        prodex_log_retention_candidates_v1(
            ABI_VERSION,
            operation,
            candidate_count_u64,
            candidate_rows.as_ptr() as usize as u64,
            file_names.as_ptr() as usize as u64,
            file_names_length,
            policy.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(output.into_iter().take(candidate_count).collect())
}

fn encode_retention_candidates(
    candidates: &[LogRetentionCandidate<'_>],
    policy: LogRetentionCandidatePolicy,
) -> Result<(Vec<u64>, Vec<u8>), MojoError> {
    let row_capacity = candidates
        .len()
        .checked_mul(RETENTION_CANDIDATE_STRIDE)
        .ok_or(MojoError::InvalidInput)?;
    let names_capacity = candidates
        .iter()
        .try_fold(0_usize, |total, candidate| {
            total.checked_add(candidate.file_name.len())
        })
        .ok_or(MojoError::InvalidInput)?;
    if candidates
        .iter()
        .any(|candidate| candidate.file_name.is_empty())
    {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::with_capacity(row_capacity);
    let mut file_names = Vec::with_capacity(names_capacity);
    for candidate in candidates {
        let name_offset = u64::try_from(file_names.len()).map_err(|_| MojoError::InvalidInput)?;
        let name_length =
            u64::try_from(candidate.file_name.len()).map_err(|_| MojoError::InvalidInput)?;
        rows.extend([
            candidate.size,
            signed_order_key(candidate.modified_epoch_seconds),
            u64::from(candidate.removable)
                | (u64::from(
                    matches!(policy, LogRetentionCandidatePolicy::OverBudget { .. })
                        && candidate.unavailable,
                ) << 1),
            name_offset,
            name_length,
        ]);
        file_names.extend_from_slice(candidate.file_name);
    }
    Ok((rows, file_names))
}

/// Select expired logs from candidates in caller order.
pub fn log_expired_candidate_plan(
    candidates: &[LogRetentionCandidate<'_>],
    oldest_allowed_epoch_seconds: i64,
) -> Result<Vec<bool>, MojoError> {
    retention_candidates_call(
        candidates,
        LogRetentionCandidatePolicy::Expired {
            oldest_allowed_epoch_seconds,
        },
    )?
    .into_iter()
    .map(|value| bool_output(i64::try_from(value).map_err(|_| MojoError::InvalidOutput)?))
    .collect()
}

/// Return removable candidate indices in oldest-first order until both budgets fit.
pub fn log_over_budget_candidate_plan(
    candidates: &[LogRetentionCandidate<'_>],
    remaining_count: usize,
    max_files: usize,
    total_bytes: u64,
    total_budget: u64,
) -> Result<Vec<usize>, MojoError> {
    let output = retention_candidates_call(
        candidates,
        LogRetentionCandidatePolicy::OverBudget {
            remaining_count,
            max_files,
            total_bytes,
            total_budget,
        },
    )?;
    let mut selected = Vec::new();
    let mut seen = vec![false; candidates.len()];
    for value in output {
        if value == u64::MAX {
            break;
        }
        let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
        if index >= seen.len() || seen[index] {
            return Err(MojoError::InvalidOutput);
        }
        seen[index] = true;
        selected.push(index);
    }
    Ok(selected)
}

pub fn sample_plan(
    previous: Option<(u64, u64)>,
    current_tokens: u64,
    current_generation_ms: u64,
) -> Result<ThroughputSamplePlan, MojoError> {
    let (previous_present, previous_tokens, previous_generation_ms) = previous
        .map(|(tokens, generation_ms)| (1_i64, tokens, generation_ms))
        .unwrap_or((0_i64, 0, 0));
    let mut output = [-1_i64; 2];
    status(unsafe {
        prodex_log_throughput_sample_plan_v1(
            ABI_VERSION,
            previous_present,
            previous_tokens,
            previous_generation_ms,
            current_tokens,
            current_generation_ms,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(ThroughputSamplePlan {
        counter_reset: bool_output(output[0])?,
        append_sample: bool_output(output[1])?,
    })
}

fn rate_output(status_code: i64, valid: i64, rate: f64) -> Result<Option<f64>, MojoError> {
    status(status_code)?;
    match valid {
        0 => Ok(None),
        1 if rate.is_finite() && rate > 0.0 => Ok(Some(rate)),
        1 => Err(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn completed_rate(output_tokens: u64, generation_ms: u64) -> Result<Option<f64>, MojoError> {
    let mut valid = -1_i64;
    let mut rate = 0.0_f64;
    let status_code = unsafe {
        prodex_log_throughput_completed_rate_v1(
            ABI_VERSION,
            output_tokens,
            generation_ms,
            (&mut valid as *mut i64) as usize as u64,
            (&mut rate as *mut f64) as usize as u64,
        )
    };
    rate_output(status_code, valid, rate)
}

pub fn stream_rate(
    first_tokens: u64,
    first_generation_ms: u64,
    last_tokens: u64,
    last_generation_ms: u64,
) -> Result<Option<f64>, MojoError> {
    let mut valid = -1_i64;
    let mut rate = 0.0_f64;
    let status_code = unsafe {
        prodex_log_throughput_stream_rate_v1(
            ABI_VERSION,
            first_tokens,
            first_generation_ms,
            last_tokens,
            last_generation_ms,
            (&mut valid as *mut i64) as usize as u64,
            (&mut rate as *mut f64) as usize as u64,
        )
    };
    rate_output(status_code, valid, rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throughput_policy_kernel_preserves_counter_and_rate_contract() {
        assert_eq!(
            sample_plan(Some((100, 1000)), 99, 1100).unwrap(),
            ThroughputSamplePlan {
                counter_reset: true,
                append_sample: true,
            }
        );
        assert_eq!(
            sample_plan(Some((100, 1000)), 100, 1100).unwrap(),
            ThroughputSamplePlan {
                counter_reset: false,
                append_sample: false,
            }
        );
        assert_eq!(
            sample_plan(Some((100, 1000)), 101, 900).unwrap(),
            ThroughputSamplePlan {
                counter_reset: true,
                append_sample: true,
            }
        );
        assert_eq!(completed_rate(0, 1000).unwrap(), None);
        assert_eq!(completed_rate(100, 0).unwrap(), None);
        assert_eq!(completed_rate(100, 2000).unwrap(), Some(50.0));
        assert_eq!(stream_rate(100, 1000, 200, 3000).unwrap(), Some(50.0));
        assert_eq!(stream_rate(100, 1000, 110, 1200).unwrap(), None);
        assert_eq!(stream_rate(200, 1000, 100, 3000).unwrap(), None);
        assert_eq!(stream_rate(100, 3000, 200, 1000).unwrap(), None);
        assert_eq!(bounded_policy_value(Some(9), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_policy_value(Some(7), 5, 1, 8).unwrap(), 7);
        assert_eq!(bounded_text_policy_value(None, 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some("7"), 5, 1, 8).unwrap(), 7);
        assert_eq!(bounded_text_policy_value(Some("+7"), 5, 1, 8).unwrap(), 7);
        assert_eq!(bounded_text_policy_value(Some(" 7"), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some("7 "), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some("-1"), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some(""), 5, 1, 8).unwrap(), 5);
        assert_eq!(bounded_text_policy_value(Some("09"), 5, 1, 10).unwrap(), 9);
        assert_eq!(
            bounded_text_policy_value(Some("18446744073709551615"), 5, 1, u64::MAX).unwrap(),
            u64::MAX
        );
        assert_eq!(
            bounded_text_policy_value(Some("18446744073709551616"), 5, 1, u64::MAX).unwrap(),
            5
        );
        assert_eq!(
            log_rotation_plan(10, 5, 12).unwrap(),
            LogRotationPlan {
                rotate_before_write: true,
                rotate_after_oversized_line: false,
            }
        );
    }

    #[test]
    fn runtime_log_retention_abi_selects_expired_and_oldest_budget_candidates() {
        let candidates = [
            LogRetentionCandidate {
                file_name: b"prodex-runtime-z-old.log",
                size: 6,
                modified_epoch_seconds: 99,
                removable: true,
                unavailable: false,
            },
            LogRetentionCandidate {
                file_name: b"prodex-runtime-b-cutoff.log",
                size: 5,
                modified_epoch_seconds: 100,
                removable: true,
                unavailable: false,
            },
            LogRetentionCandidate {
                file_name: b"prodex-runtime-c-new.log",
                size: 4,
                modified_epoch_seconds: 101,
                removable: true,
                unavailable: false,
            },
            LogRetentionCandidate {
                file_name: b"prodex-runtime-a-protected.log",
                size: 3,
                modified_epoch_seconds: 0,
                removable: false,
                unavailable: false,
            },
        ];
        assert_eq!(
            log_expired_candidate_plan(&candidates, 100).unwrap(),
            [true, false, false, false]
        );
        assert_eq!(log_expired_candidate_plan(&[], 100).unwrap(), []);

        let unordered = [
            LogRetentionCandidate {
                file_name: b"prodex-runtime-z-new.log",
                size: 6,
                modified_epoch_seconds: 5,
                removable: false,
                unavailable: false,
            },
            LogRetentionCandidate {
                file_name: b"prodex-runtime-b-old.log",
                size: 5,
                modified_epoch_seconds: 2,
                removable: true,
                unavailable: false,
            },
            LogRetentionCandidate {
                file_name: b"prodex-runtime-a-removed.log",
                size: 4,
                modified_epoch_seconds: 3,
                removable: true,
                unavailable: true,
            },
            LogRetentionCandidate {
                file_name: b"prodex-runtime-c-protected.log",
                size: 3,
                modified_epoch_seconds: 4,
                removable: false,
                unavailable: false,
            },
            LogRetentionCandidate {
                file_name: b"prodex-runtime-a-oldest.log",
                size: 2,
                modified_epoch_seconds: 1,
                removable: true,
                unavailable: false,
            },
        ];
        assert_eq!(
            log_over_budget_candidate_plan(&unordered, 4, 3, 16, 10).unwrap(),
            [4, 1]
        );

        let tied = [
            LogRetentionCandidate {
                file_name: b"prodex-runtime-b.log",
                size: 6,
                modified_epoch_seconds: 1,
                removable: true,
                unavailable: false,
            },
            LogRetentionCandidate {
                file_name: b"prodex-runtime-a.log",
                size: 5,
                modified_epoch_seconds: 1,
                removable: true,
                unavailable: false,
            },
        ];
        assert_eq!(
            log_over_budget_candidate_plan(&tied, 2, 1, 11, 6).unwrap(),
            [1]
        );

        let unicode_tie = [
            LogRetentionCandidate {
                file_name: "prodex-runtime-Ω.log".as_bytes(),
                size: 1,
                modified_epoch_seconds: 7,
                removable: true,
                unavailable: false,
            },
            LogRetentionCandidate {
                file_name: "prodex-runtime-é.log".as_bytes(),
                size: 1,
                modified_epoch_seconds: 7,
                removable: true,
                unavailable: false,
            },
        ];
        assert_eq!(
            log_over_budget_candidate_plan(&unicode_tie, 2, 1, 2, 1).unwrap(),
            [1]
        );
    }

    #[test]
    fn runtime_log_retention_abi_rejects_invalid_candidate_flags() {
        assert_eq!(
            retention_candidates_abi_call(
                RETENTION_OVER_BUDGET_CANDIDATES,
                &[1, 0, 4, 0, 1],
                b"a",
                [0, 1, 1, 1, 1],
            ),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            log_over_budget_candidate_plan(
                &[LogRetentionCandidate {
                    file_name: b"",
                    size: 1,
                    modified_epoch_seconds: 0,
                    removable: true,
                    unavailable: false,
                }],
                1,
                1,
                1,
                1,
            ),
            Err(MojoError::InvalidInput)
        );
    }
}
