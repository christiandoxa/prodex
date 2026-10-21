//! Mojo-owned SSE tap-state decisions for runtime response forwarding.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MODE_RESPONSE_ID: i64 = 0;
const MODE_TOKEN_USAGE: i64 = 1;

const RESPONSE_ID_RECORD: i64 = 1;
const RESPONSE_ID_RECORD_WITH_TURN_STATE: i64 = 2;
const RESPONSE_ID_BACKFILL_TURN_STATE: i64 = 4;

const TOKEN_USAGE_NONE: i64 = 0;
const TOKEN_USAGE_LOG: i64 = 1;
const TOKEN_USAGE_LOG_WITH_GENERATION: i64 = 2;

unsafe extern "C" {
    fn prodex_runtime_response_header_select_v1(
        abi_version: i64,
        operation: i64,
        rows_address: u64,
        row_count: i64,
        query_address: u64,
        query_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_response_forwarding_tap_state_plan_v1(
        abi_version: i64,
        mode: i64,
        already_remembered: i64,
        turn_state_present: i64,
        remembered_count: u64,
        bound_count: u64,
        token_usage_present: i64,
        duplicate_token_usage: i64,
        loggable_usage: i64,
        generation_present: i64,
    ) -> i64;
}

/// One original borrowed HTTP header, with a binary wire value.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeResponseHeaderInput<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum RuntimeResponseHeaderOperation {
    FirstValue = 0,
    FirstBinaryContentType = 1,
    SseChunkFlush = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseHeaderSelection {
    pub index: Option<usize>,
    pub start: usize,
    pub end: usize,
}

/// Mojo owns first-match and Unicode whitespace semantics; the host keeps
/// borrowed original buffers and only applies the chosen slice or flag.
pub fn response_header_selection(
    headers: &[RuntimeResponseHeaderInput<'_>],
    operation: RuntimeResponseHeaderOperation,
    query: Option<&str>,
) -> Result<RuntimeResponseHeaderSelection, MojoError> {
    if headers.len() > 1_048_576
        || (operation == RuntimeResponseHeaderOperation::FirstValue) != query.is_some()
    {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<u64>::new();
    rows.try_reserve_exact(headers.len().checked_mul(4).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for header in headers {
        rows.extend([
            header.name.as_ptr() as usize as u64,
            u64::try_from(header.name.len()).map_err(|_| MojoError::InvalidInput)?,
            header.value.as_ptr() as usize as u64,
            u64::try_from(header.value.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let (query_address, query_length) = query.map_or(Ok((0, 0)), |value| {
        Ok((
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        ))
    })?;
    let mut output = [-1_i64; 3];
    match unsafe {
        prodex_runtime_response_header_select_v1(
            ABI_VERSION,
            operation as i64,
            rows.as_ptr() as usize as u64,
            i64::try_from(headers.len()).map_err(|_| MojoError::InvalidInput)?,
            query_address,
            query_length,
            output.as_mut_ptr() as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    decode_response_header_selection(headers, operation, output)
}

fn decode_response_header_selection(
    headers: &[RuntimeResponseHeaderInput<'_>],
    operation: RuntimeResponseHeaderOperation,
    output: [i64; 3],
) -> Result<RuntimeResponseHeaderSelection, MojoError> {
    let index = match output[0] {
        -1 => None,
        value => Some(usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?),
    };
    if index.is_some_and(|index| index >= headers.len()) {
        return Err(MojoError::InvalidOutput);
    }
    let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if start > end || index.is_none() && (start != 0 || end != 0) {
        return Err(MojoError::InvalidOutput);
    }
    if operation == RuntimeResponseHeaderOperation::SseChunkFlush && (start != 0 || end != 0) {
        return Err(MojoError::InvalidOutput);
    }
    if let Some(index) = index {
        let value = headers[index].value;
        if end > value.len()
            || operation != RuntimeResponseHeaderOperation::SseChunkFlush
                && (start == end || std::str::from_utf8(&value[start..end]).is_err())
        {
            return Err(MojoError::InvalidOutput);
        }
    }
    Ok(RuntimeResponseHeaderSelection { index, start, end })
}

/// Facts needed to plan one response-ID update or turn-state backfill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseForwardingResponseIdInput {
    pub already_remembered: bool,
    pub turn_state_present: bool,
    pub remembered_count: u64,
    pub bound_count: u64,
}

/// Host effects selected for one response-ID update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseForwardingResponseIdPlan {
    pub record: bool,
    pub record_with_turn_state: bool,
    pub backfill_turn_state: bool,
}

/// Facts needed to plan one token-usage logging effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseForwardingTokenUsageInput {
    pub token_usage_present: bool,
    pub duplicate_token_usage: bool,
    pub loggable_usage: bool,
    pub generation_present: bool,
}

/// Host effect selected for one token-usage observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeResponseForwardingTokenUsagePlan {
    None,
    Log,
    LogWithGeneration,
}

fn call(
    mode: i64,
    already_remembered: bool,
    turn_state_present: bool,
    remembered_count: u64,
    bound_count: u64,
    input: RuntimeResponseForwardingTokenUsageInput,
) -> Result<i64, MojoError> {
    let result = unsafe {
        prodex_runtime_response_forwarding_tap_state_plan_v1(
            ABI_VERSION,
            mode,
            i64::from(already_remembered),
            i64::from(turn_state_present),
            remembered_count,
            bound_count,
            i64::from(input.token_usage_present),
            i64::from(input.duplicate_token_usage),
            i64::from(input.loggable_usage),
            i64::from(input.generation_present),
        )
    };
    match result {
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        value => Ok(value),
    }
}

/// Select response-ID recording and turn-state backfill effects.
pub fn response_id_plan(
    input: RuntimeResponseForwardingResponseIdInput,
) -> Result<RuntimeResponseForwardingResponseIdPlan, MojoError> {
    let result = call(
        MODE_RESPONSE_ID,
        input.already_remembered,
        input.turn_state_present,
        input.remembered_count,
        input.bound_count,
        RuntimeResponseForwardingTokenUsageInput {
            token_usage_present: false,
            duplicate_token_usage: false,
            loggable_usage: false,
            generation_present: false,
        },
    )?;
    if !(0..=7).contains(&result) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RuntimeResponseForwardingResponseIdPlan {
        record: result & RESPONSE_ID_RECORD != 0,
        record_with_turn_state: result & RESPONSE_ID_RECORD_WITH_TURN_STATE != 0,
        backfill_turn_state: result & RESPONSE_ID_BACKFILL_TURN_STATE != 0,
    })
}

/// Select the deduplicated token-usage logging effect.
pub fn token_usage_plan(
    input: RuntimeResponseForwardingTokenUsageInput,
) -> Result<RuntimeResponseForwardingTokenUsagePlan, MojoError> {
    let result = call(MODE_TOKEN_USAGE, false, false, 0, 0, input)?;
    match result {
        TOKEN_USAGE_NONE => Ok(RuntimeResponseForwardingTokenUsagePlan::None),
        TOKEN_USAGE_LOG => Ok(RuntimeResponseForwardingTokenUsagePlan::Log),
        TOKEN_USAGE_LOG_WITH_GENERATION => {
            Ok(RuntimeResponseForwardingTokenUsagePlan::LogWithGeneration)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_id_input() -> RuntimeResponseForwardingResponseIdInput {
        RuntimeResponseForwardingResponseIdInput {
            already_remembered: false,
            turn_state_present: true,
            remembered_count: 2,
            bound_count: 1,
        }
    }

    fn token_usage_input() -> RuntimeResponseForwardingTokenUsageInput {
        RuntimeResponseForwardingTokenUsageInput {
            token_usage_present: true,
            duplicate_token_usage: false,
            loggable_usage: true,
            generation_present: true,
        }
    }

    #[test]
    fn compiled_response_header_selection_preserves_first_match_and_utf8_aliases() {
        use RuntimeResponseHeaderOperation::{FirstBinaryContentType, FirstValue, SseChunkFlush};
        let text = [
            RuntimeResponseHeaderInput {
                name: "x-value",
                value: " \u{3000} \t".as_bytes(),
            },
            RuntimeResponseHeaderInput {
                name: "X-Value",
                value: b"later",
            },
        ];
        assert_eq!(
            response_header_selection(&text, FirstValue, Some("X-VALUE")),
            Ok(RuntimeResponseHeaderSelection {
                index: None,
                start: 0,
                end: 0
            }),
        );
        let text = [
            RuntimeResponseHeaderInput {
                name: "X-Value",
                value: " \u{3000}selected 雪 \t".as_bytes(),
            },
            RuntimeResponseHeaderInput {
                name: "x-value",
                value: b"later",
            },
        ];
        let choice = response_header_selection(&text, FirstValue, Some("x-value")).unwrap();
        assert_eq!(choice.index, Some(0));
        assert_eq!(
            std::str::from_utf8(&text[0].value[choice.start..choice.end]),
            Ok("selected 雪"),
        );
        assert_eq!(
            response_header_selection(&text, FirstValue, Some("not-present"))
                .unwrap()
                .index,
            None,
        );
        assert_eq!(
            response_header_selection(&text, FirstValue, None),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            response_header_selection(&text, SseChunkFlush, Some("value")),
            Err(MojoError::InvalidInput),
        );

        let binary = [
            RuntimeResponseHeaderInput {
                name: "Content-Type",
                value: &[255_u8, 250],
            },
            RuntimeResponseHeaderInput {
                name: " CONTENT-TYPE ",
                value: b"   ",
            },
            RuntimeResponseHeaderInput {
                name: "content-type",
                value: b"  application/json ",
            },
        ];
        let choice = response_header_selection(&binary, FirstBinaryContentType, None).unwrap();
        assert_eq!(choice.index, Some(2));
        assert_eq!(
            &binary[2].value[choice.start..choice.end],
            b"application/json"
        );
        assert_eq!(
            response_header_selection(&binary, SseChunkFlush, None)
                .unwrap()
                .index,
            None,
        );
        let sse = [
            RuntimeResponseHeaderInput {
                name: "x-other",
                value: b"text/event-stream",
            },
            RuntimeResponseHeaderInput {
                name: "CONTENT-TYPE",
                value: b"TEXT/EVENT-STREAM; charset=utf-8",
            },
        ];
        assert_eq!(
            response_header_selection(&sse, SseChunkFlush, None).unwrap(),
            RuntimeResponseHeaderSelection {
                index: Some(1),
                start: 0,
                end: 0
            },
        );
        assert_eq!(
            response_header_selection(&[], SseChunkFlush, None)
                .unwrap()
                .index,
            None,
        );
    }

    #[test]
    fn compiled_response_header_selection_abi_rejects_bad_late_rows_without_writing() {
        let mut result = [79_i64; 3];
        let target = result.as_mut_ptr() as usize as u64;
        let mut fields = [0_u64; 8];
        let rows = fields.as_ptr() as usize as u64;
        for (version, mode, rows, count, query, query_len, expected) in [
            (2, 0, rows, 2, 0, 0, 4),
            (1, 3, rows, 2, 0, 0, 1),
            (1, 0, 0, 2, 0, 0, 1),
            (1, 0, rows, -1, 0, 0, 1),
            (1, 0, rows, 1_048_577, 0, 0, 1),
            (1, 2, rows, 2, 2, 1, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_response_header_select_v1(
                        version, mode, rows, count, query, query_len, target,
                    )
                },
                expected,
            );
            assert_eq!(result, [79; 3]);
        }
        fields[7] = u64::MAX;
        assert_eq!(
            unsafe {
                prodex_runtime_response_header_select_v1(
                    1,
                    1,
                    fields.as_ptr() as usize as u64,
                    2,
                    0,
                    0,
                    target,
                )
            },
            1,
        );
        assert_eq!(result, [79; 3]);
    }

    #[test]
    fn response_id_plan_preserves_record_and_backfill_precedence() {
        assert_eq!(
            response_id_plan(response_id_input()).unwrap(),
            RuntimeResponseForwardingResponseIdPlan {
                record: true,
                record_with_turn_state: true,
                backfill_turn_state: true,
            }
        );
        let mut bound = response_id_input();
        bound.bound_count = 2;
        assert!(!response_id_plan(bound).unwrap().backfill_turn_state);
        let mut existing = response_id_input();
        existing.already_remembered = true;
        assert!(!response_id_plan(existing).unwrap().record);
    }

    #[test]
    fn token_usage_plan_deduplicates_and_preserves_generation_detail() {
        assert_eq!(
            token_usage_plan(token_usage_input()).unwrap(),
            RuntimeResponseForwardingTokenUsagePlan::LogWithGeneration
        );
        let mut plain = token_usage_input();
        plain.generation_present = false;
        assert_eq!(
            token_usage_plan(plain).unwrap(),
            RuntimeResponseForwardingTokenUsagePlan::Log
        );
        let mut duplicate = token_usage_input();
        duplicate.duplicate_token_usage = true;
        assert_eq!(
            token_usage_plan(duplicate).unwrap(),
            RuntimeResponseForwardingTokenUsagePlan::None
        );
    }

    #[test]
    fn tap_state_plans_change_for_each_semantic_fact() {
        let base = response_id_input();
        let mutators: &[fn(&mut RuntimeResponseForwardingResponseIdInput)] = &[
            |input| input.already_remembered = true,
            |input| input.turn_state_present = false,
            |input| input.remembered_count = 1,
            |input| input.bound_count = 2,
        ];
        let expected = response_id_plan(base).unwrap();
        for mutate in mutators {
            let mut candidate = base;
            mutate(&mut candidate);
            assert_ne!(response_id_plan(candidate).unwrap(), expected);
        }

        let base = token_usage_input();
        let mutators: &[fn(&mut RuntimeResponseForwardingTokenUsageInput)] = &[
            |input| input.token_usage_present = false,
            |input| input.duplicate_token_usage = true,
            |input| input.loggable_usage = false,
            |input| input.generation_present = false,
        ];
        let expected = token_usage_plan(base).unwrap();
        for mutate in mutators {
            let mut candidate = base;
            mutate(&mut candidate);
            assert_ne!(token_usage_plan(candidate).unwrap(), expected);
        }
    }

    #[test]
    fn tap_state_abi_rejects_bad_version_and_counts() {
        assert_eq!(
            unsafe {
                prodex_runtime_response_forwarding_tap_state_plan_v1(
                    ABI_VERSION + 1,
                    MODE_RESPONSE_ID,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                )
            },
            -4
        );
        assert_eq!(
            response_id_plan(RuntimeResponseForwardingResponseIdInput {
                remembered_count: 1,
                bound_count: 2,
                ..response_id_input()
            }),
            Err(MojoError::InvalidInput)
        );
    }
}
