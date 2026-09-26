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
    fn prodex_mojo_log_parse_v1(
        message_address: u64,
        message_length: i64,
        records_address: u64,
        record_capacity: i64,
        result_address: u64,
    ) -> i64;
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
