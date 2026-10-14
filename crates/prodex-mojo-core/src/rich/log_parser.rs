use crate::MojoError;

const LOG_FIELD_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogFieldSpan {
    pub key_start: usize,
    pub key_end: usize,
    pub value_start: usize,
    pub value_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogParsePlan {
    pub event: Option<(usize, usize)>,
    pub fields: Vec<LogFieldSpan>,
}

unsafe extern "C" {
    fn prodex_mojo_log_plain_envelope_v1(
        abi_version: i64,
        line_address: u64,
        line_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_mojo_log_parse_v1(
        message_address: u64,
        message_length: i64,
        records_address: u64,
        record_capacity: i64,
        result_address: u64,
    ) -> i64;
}

/// Borrowed UTF-8 ranges from the compiled Mojo delimiter plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlainLogEnvelope<'a> {
    pub timestamp: Option<&'a str>,
    pub message: &'a str,
}

pub fn parse_plain_log_envelope(line: &str) -> Result<PlainLogEnvelope<'_>, MojoError> {
    let mut result = [-1_i64; 3];
    let status = unsafe {
        prodex_mojo_log_plain_envelope_v1(
            1,
            line.as_ptr() as usize as u64,
            i64::try_from(line.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
            3,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }

    let message_start = usize::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?;
    let message = line.get(message_start..).ok_or(MojoError::InvalidOutput)?;
    let timestamp = if result[0] == -1 && result[1] == 0 {
        None
    } else {
        let start = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
        let end = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
        if start != 1 || end > message_start {
            return Err(MojoError::InvalidOutput);
        }
        Some(line.get(start..end).ok_or(MojoError::InvalidOutput)?)
    };
    Ok(PlainLogEnvelope { timestamp, message })
}

pub fn parse_log_message(message: &str) -> Result<LogParsePlan, MojoError> {
    let message_len = message.len();
    let record_capacity = message_len / 2 + 1;
    let mut records = vec![0_i64; record_capacity.saturating_mul(LOG_FIELD_WIDTH)];
    let mut result = [-1_i64, 0_i64, 0_i64];
    let status = unsafe {
        prodex_mojo_log_parse_v1(
            message.as_ptr() as usize as u64,
            i64::try_from(message_len).map_err(|_| MojoError::InvalidInput)?,
            records.as_mut_ptr() as usize as u64,
            i64::try_from(record_capacity).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::Capacity,
            _ => MojoError::InvalidOutput,
        });
    }
    let field_count = usize::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?;
    if field_count > record_capacity {
        return Err(MojoError::InvalidOutput);
    }

    let event = if result[0] < 0 {
        None
    } else {
        let start = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
        let len = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
        let end = start.checked_add(len).ok_or(MojoError::InvalidOutput)?;
        validate_span(message, start, end)?;
        Some((start, end))
    };

    let mut fields = Vec::with_capacity(field_count);
    for index in 0..field_count {
        let base = index * LOG_FIELD_WIDTH;
        let key_start = usize::try_from(records[base]).map_err(|_| MojoError::InvalidOutput)?;
        let key_end = usize::try_from(records[base + 1]).map_err(|_| MojoError::InvalidOutput)?;
        let value_start =
            usize::try_from(records[base + 2]).map_err(|_| MojoError::InvalidOutput)?;
        let value_end = usize::try_from(records[base + 3]).map_err(|_| MojoError::InvalidOutput)?;
        validate_span(message, key_start, key_end)?;
        validate_span(message, value_start, value_end)?;
        if key_start == key_end || value_start == value_end || key_end >= value_start {
            return Err(MojoError::InvalidOutput);
        }
        fields.push(LogFieldSpan {
            key_start,
            key_end,
            value_start,
            value_end,
        });
    }
    Ok(LogParsePlan { event, fields })
}

fn validate_span(message: &str, start: usize, end: usize) -> Result<(), MojoError> {
    if start <= end
        && end <= message.len()
        && message.is_char_boundary(start)
        && message.is_char_boundary(end)
    {
        Ok(())
    } else {
        Err(MojoError::InvalidOutput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_log_envelope_decides_first_delimiter_for_utf8_and_plain_text() {
        for (line, timestamp, message) in [
            (
                "[2026-10-14T10:00:00Z] ✅ ready",
                Some("2026-10-14T10:00:00Z"),
                "✅ ready",
            ),
            ("[x]  next] later", Some("x"), " next] later"),
            ("prefix] hello", None, "hello"),
            ("[unterminated", None, "[unterminated"),
            ("[] ", Some(""), ""),
            ("[ ] ", Some(" "), ""),
            ("", None, ""),
            ("🙂 ] message", None, "message"),
        ] {
            let actual = parse_plain_log_envelope(line).expect("compiled Mojo envelope");
            assert_eq!(actual.timestamp, timestamp, "{line:?}");
            assert_eq!(actual.message, message, "{line:?}");
        }
    }

    #[test]
    fn plain_log_envelope_rejects_invalid_abi_and_capacity() {
        let line = b"[x] msg";
        let mut slots = [7i64; 3];
        let source = line.as_ptr() as usize as u64;
        let result = slots.as_mut_ptr() as usize as u64;
        unsafe {
            assert_eq!(
                prodex_mojo_log_plain_envelope_v1(2, source, 7, result, 3),
                4
            );
            assert_eq!(
                prodex_mojo_log_plain_envelope_v1(1, source, 7, result, 2),
                1
            );
            assert_eq!(prodex_mojo_log_plain_envelope_v1(1, 0, 7, result, 3), 1);
        }
        assert_eq!(slots, [7; 3]);
    }

    #[test]
    fn form_feed_separates_runtime_log_tokens() {
        let message = "\u{c}event\u{c}key=value\u{c}next=field";
        let plan = parse_log_message(message).unwrap();
        assert_eq!(
            plan.event.map(|(start, end)| &message[start..end]),
            Some("event")
        );
        assert_eq!(
            plan.fields
                .iter()
                .map(|field| (
                    &message[field.key_start..field.key_end],
                    &message[field.value_start..field.value_end],
                ))
                .collect::<Vec<_>>(),
            [("key", "value"), ("next", "field")]
        );
    }
}
