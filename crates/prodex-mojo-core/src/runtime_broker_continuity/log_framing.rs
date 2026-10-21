//! Mojo-owned bounded binary log framing decisions for broker continuity.
//! Host retains BufReader, Vec buffers, parsed metrics and update effects.

use super::*;
const ABI_VERSION: i64 = 1;
const MAX_SCAN_BYTES: usize = 64 * 1024 * 1024;

unsafe extern "C" {
    fn prodex_runtime_broker_log_frame_plan_v1(
        abi_version: i64,
        data_address: u64,
        data_length: i64,
        buffered_line_length: i64,
        already_oversized: i64,
        output_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerLogFrameAction {
    Ignore,
    Append,
    ClearOversized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerLogFramePlan {
    pub take: usize,
    pub complete: bool,
    pub action: BrokerLogFrameAction,
    pub oversized_after: bool,
    pub emit_metrics: bool,
}

/// Determine first complete LF consumption, bounded buffering action and
/// downstream metric eligibility from the exact borrowed binary chunk.
pub fn broker_log_frame_plan(
    available: &[u8],
    pending_line_len: usize,
    already_oversized: bool,
) -> Result<BrokerLogFramePlan, MojoError> {
    if available.len() > MAX_SCAN_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 5];
    let code = unsafe {
        prodex_runtime_broker_log_frame_plan_v1(
            ABI_VERSION,
            available.as_ptr() as usize as u64,
            i64::try_from(available.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(pending_line_len).map_err(|_| MojoError::InvalidInput)?,
            i64::from(already_oversized),
            output.as_mut_ptr() as usize as u64,
        )
    };
    match code {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let take = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if take > available.len() {
        return Err(MojoError::InvalidOutput);
    }
    let decode_flag = |flag| match flag {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let complete = decode_flag(output[1])?;
    let action = match output[2] {
        0 => BrokerLogFrameAction::Ignore,
        1 => BrokerLogFrameAction::Append,
        2 => BrokerLogFrameAction::ClearOversized,
        _ => return Err(MojoError::InvalidOutput),
    };
    let oversized_after = decode_flag(output[3])?;
    let emit_metrics = decode_flag(output[4])?;
    if (complete && (take == 0 || available[take - 1] != b'\n'))
        || (!complete && take != available.len())
        || (complete && emit_metrics && oversized_after)
        || (action == BrokerLogFrameAction::ClearOversized && !oversized_after)
        || (already_oversized && action != BrokerLogFrameAction::Ignore)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(BrokerLogFramePlan {
        take,
        complete,
        action,
        oversized_after,
        emit_metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_broker_log_frame_scans_binary_and_partial_line_boundaries() {
        let expected = BrokerLogFramePlan {
            take: 4,
            complete: true,
            action: BrokerLogFrameAction::Append,
            oversized_after: false,
            emit_metrics: true,
        };
        assert_eq!(
            broker_log_frame_plan(b"one\n\xff\ntrailing", 0, false),
            Ok(expected),
        );
        assert_eq!(
            broker_log_frame_plan(b"\xff\ntrailing", 0, false),
            Ok(BrokerLogFramePlan {
                take: 2,
                ..expected
            }),
        );
        assert_eq!(
            broker_log_frame_plan(b"partial", 0, false),
            Ok(BrokerLogFramePlan {
                take: 7,
                complete: false,
                emit_metrics: false,
                ..expected
            }),
        );
        assert_eq!(
            broker_log_frame_plan(b"", 0, false),
            Ok(BrokerLogFramePlan {
                take: 0,
                complete: false,
                emit_metrics: false,
                ..expected
            }),
        );
        assert_eq!(
            broker_log_frame_plan(b"\ntrailing", 0, false),
            Ok(BrokerLogFramePlan {
                take: 1,
                ..expected
            }),
        );
    }

    #[test]
    fn compiled_broker_log_frame_enforces_exact_megabyte_cap_and_recovers() {
        const LIMIT: usize = 1024 * 1024;
        let full = broker_log_frame_plan(b"\n", LIMIT - 1, false).unwrap();
        assert_eq!(full.action, BrokerLogFrameAction::Append);
        assert!(full.emit_metrics);
        assert!(!full.oversized_after);

        let exceeded = broker_log_frame_plan(b"\n", LIMIT, false).unwrap();
        assert_eq!(exceeded.action, BrokerLogFrameAction::ClearOversized);
        assert!(exceeded.complete);
        assert!(exceeded.oversized_after);
        assert!(!exceeded.emit_metrics);

        let partially_exceeded = broker_log_frame_plan(b"tail", LIMIT, false).unwrap();
        assert_eq!(
            partially_exceeded.action,
            BrokerLogFrameAction::ClearOversized
        );
        assert!(!partially_exceeded.complete);
        assert_eq!(
            broker_log_frame_plan(b"ignored\nnew", 0, true).unwrap(),
            BrokerLogFramePlan {
                take: 8,
                complete: true,
                action: BrokerLogFrameAction::Ignore,
                oversized_after: true,
                emit_metrics: false,
            },
        );
        assert!(
            broker_log_frame_plan(b"next\n", 0, false)
                .unwrap()
                .emit_metrics
        );
    }

    #[test]
    fn compiled_broker_log_frame_abi_rejects_bad_inputs_without_modifying_output() {
        let bytes = b"hello\n";
        let mut output = [73_i64; 5];
        let source = bytes.as_ptr() as usize as u64;
        let target = output.as_mut_ptr() as usize as u64;
        for (version, ptr, len, line_len, oversized, dest, expected) in [
            (2, source, 6, 0, 0, target, 4),
            (1, 0, 6, 0, 0, target, 1),
            (1, source, -1, 0, 0, target, 1),
            (1, source, 64 * 1024 * 1024 + 1, 0, 0, target, 1),
            (1, source, 6, -1, 0, target, 1),
            (1, source, 6, 0, 2, target, 1),
            (1, source, 6, 0, 0, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_broker_log_frame_plan_v1(
                        version, ptr, len, line_len, oversized, dest,
                    )
                },
                expected,
            );
            assert_eq!(output, [73; 5]);
        }
    }
}
