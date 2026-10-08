use super::*;

const RUNTIME_RESPONSE_EVENT_KIND_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_runtime_response_metadata_event_kind_v1(
        abi_version: i64,
        event_address: u64,
        event_length: i64,
        event_present: i64,
        output_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeResponseMetadataJsonPlan {
    pub response_ids: [Option<(usize, usize)>; 3],
    pub event_type: Option<(usize, usize)>,
    pub turn_state: Option<(usize, usize)>,
    pub headers_turn_state: Option<(usize, usize)>,
    pub token_usage_present: bool,
    pub token_usage_spans: [Option<(usize, usize)>; 4],
}

pub fn runtime_response_event_is_completed(event_type: Option<&str>) -> Result<bool, MojoError> {
    let event_present = i64::from(event_type.is_some());
    let event_type = event_type.unwrap_or_default();
    let mut output = [-1_i64; 1];
    status(unsafe {
        prodex_runtime_response_metadata_event_kind_v1(
            RUNTIME_RESPONSE_EVENT_KIND_ABI_VERSION,
            event_type.as_ptr() as u64,
            signed(event_type.len())?,
            event_present,
            output.as_mut_ptr() as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn validated_optional_number_span(
    start: i64,
    end: i64,
    raw: &str,
) -> Result<Option<(usize, usize)>, MojoError> {
    if start == -1 && end == -1 {
        return Ok(None);
    }
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    if end <= start || end > raw.len() {
        return Err(MojoError::InvalidOutput);
    }
    let token = raw.get(start..end).ok_or(MojoError::InvalidOutput)?;
    if token.is_empty() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some((start, end)))
}

pub fn runtime_response_metadata_json(
    raw: &str,
) -> Result<RuntimeResponseMetadataJsonPlan, MojoError> {
    let mut output = [-1_i64; 22];
    status(unsafe {
        prodex_runtime_response_metadata_json_v1(
            1,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            output.as_mut_ptr() as u64,
        )
    })?;

    let response_id_count = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if response_id_count > 3 {
        return Err(MojoError::InvalidOutput);
    }
    let mut response_ids = [None; 3];
    for (slot, output_slot) in response_ids.iter_mut().enumerate() {
        let span = validated_optional_raw_span(output[1 + slot * 2], output[2 + slot * 2], raw)?;
        if (slot < response_id_count) != span.is_some() {
            return Err(MojoError::InvalidOutput);
        }
        *output_slot = span;
    }

    let event_type = validated_optional_raw_span(output[7], output[8], raw)?;
    let turn_state = validated_optional_raw_span(output[9], output[10], raw)?;
    let headers_turn_state = validated_optional_raw_span(output[11], output[12], raw)?;
    let token_usage_present = match output[13] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut token_usage_spans = [None; 4];
    for (slot, output_slot) in token_usage_spans.iter_mut().enumerate() {
        *output_slot =
            validated_optional_number_span(output[14 + slot * 2], output[15 + slot * 2], raw)?;
    }
    if token_usage_present != token_usage_spans.iter().any(Option::is_some) {
        return Err(MojoError::InvalidOutput);
    }

    Ok(RuntimeResponseMetadataJsonPlan {
        response_ids,
        event_type,
        turn_state,
        headers_turn_state,
        token_usage_present,
        token_usage_spans,
    })
}

#[cfg(test)]
mod response_metadata_tests {
    use super::runtime_response_metadata_json;

    fn token(raw: &str, span: Option<(usize, usize)>) -> Option<&str> {
        span.map(|(start, end)| &raw[start..end])
    }

    #[test]
    fn raw_response_metadata_keeps_precedence_trim_and_usage() {
        let raw = r#"{
            "type":" response.completed ",
            "response":{
                "id":"resp-a",
                "headers":{"X-CODEX-TURN-STATE":["\t turn-a \n"]},
                "usage":{
                    "input_tokens":41,
                    "input_tokens_details":{"cached_tokens":11},
                    "output_tokens":9,
                    "output_tokens_details":{"reasoning_tokens":3}
                }
            },
            "response_id":"resp-a",
            "object":"response",
            "id":"resp-a"
        }"#;
        let plan = runtime_response_metadata_json(raw).unwrap();

        assert_eq!(token(raw, plan.response_ids[0]), Some(r#""resp-a""#));
        assert_eq!(plan.response_ids[1], None);
        assert_eq!(
            token(raw, plan.event_type),
            Some(r#"" response.completed ""#)
        );
        assert_eq!(token(raw, plan.turn_state), Some(r#""\t turn-a \n""#));
        assert_eq!(plan.headers_turn_state, None);
        assert!(plan.token_usage_present);
        let numbers = plan
            .token_usage_spans
            .map(|span| span.map(|(start, end)| raw[start..end].to_string()));
        assert_eq!(
            numbers,
            [
                Some("41".to_string()),
                Some("11".to_string()),
                Some("9".to_string()),
                Some("3".to_string())
            ]
        );

        let empty = runtime_response_metadata_json("{}").unwrap();
        assert_eq!(empty.response_ids, [None; 3]);
        assert_eq!(empty.event_type, None);
        assert_eq!(empty.turn_state, None);
        assert_eq!(empty.headers_turn_state, None);
        assert!(!empty.token_usage_present);
        assert_eq!(empty.token_usage_spans, [None; 4]);
    }
}
