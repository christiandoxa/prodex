use crate::MojoError;

const ABI_VERSION: i64 = 1;
const OP_STRIP_ENCRYPTED: i64 = 1;
const OP_AGENT_INPUT: i64 = 2;
const OP_VISION_INPUT: i64 = 3;
const OP_RESPONSE_ID: i64 = 4;

unsafe extern "C" {
    fn prodex_copilot_request_policy_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        result_address: u64,
    ) -> i64;
}

fn signed(value: usize) -> Result<i64, MojoError> {
    i64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

fn run(body: &[u8], operation: i64) -> Result<(Vec<u8>, bool), MojoError> {
    let mut output = if matches!(operation, OP_STRIP_ENCRYPTED | OP_RESPONSE_ID) {
        vec![0_u8; body.len().max(1)]
    } else {
        Vec::new()
    };
    let mut written = 0_i64;
    let mut result = 0_i64;
    let status = unsafe {
        prodex_copilot_request_policy_v1(
            ABI_VERSION,
            operation,
            body.as_ptr() as usize as u64,
            signed(body.len())?,
            if output.is_empty() {
                0
            } else {
                output.as_mut_ptr() as usize as u64
            },
            signed(output.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut result as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if !matches!(result, 0 | 1) {
        return Err(MojoError::InvalidOutput);
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok((output, result == 1))
}

pub fn strip_encrypted_content(body: &[u8]) -> Result<Option<Vec<u8>>, MojoError> {
    let (body, changed) = run(body, OP_STRIP_ENCRYPTED)?;
    Ok(changed.then_some(body))
}

pub fn has_agent_input(body: &[u8]) -> Result<bool, MojoError> {
    let (output, result) = run(body, OP_AGENT_INPUT)?;
    if !output.is_empty() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(result)
}

pub fn has_vision_input(body: &[u8]) -> Result<bool, MojoError> {
    let (output, result) = run(body, OP_VISION_INPUT)?;
    if !output.is_empty() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(result)
}

pub fn response_id(body: &[u8]) -> Result<Option<String>, MojoError> {
    let (output, present) = run(body, OP_RESPONSE_ID)?;
    if !present {
        return Ok(None);
    }
    String::from_utf8(output)
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_preserves_compaction_and_strips_other_encrypted_content() {
        let body = br#"{"input":[{"type":"compaction","encrypted_content":"keep"}],"other":{"encrypted_content":"drop"}}"#;
        let stripped = strip_encrypted_content(body).unwrap().unwrap();
        assert_eq!(
            stripped,
            br#"{"input":[{"type":"compaction","encrypted_content":"keep"}],"other":{}}"#
        );
    }

    #[test]
    fn policy_preserves_agent_and_vision_shape_semantics() {
        assert!(has_agent_input(br#"{"messages":[{"role":"ASSISTANT"}]}"#).unwrap());
        assert!(!has_agent_input(br#"{"messages":[{"role":" ASSISTANT "}]}"#).unwrap());
        assert!(has_agent_input(br#"{"input":[{"role":" \u2003ASSISTANT\u2003"}]}"#).unwrap());
        assert!(
            has_vision_input(br#"{"input":[{"type":"input_image","file_id":" file-1 "}]}"#)
                .unwrap()
        );
        assert!(!has_vision_input(
            br#"{"messages":[{"role":"assistant","content":[{"type":"image_url","image_url":{"url":"https://example.test/x.png"}}]}]}"#
        )
        .unwrap());
    }
}
