use crate::MojoError;

const REDACTION_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_redaction_json_field_plan_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_redaction_key_sensitive_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
    ) -> i64;
    fn prodex_redaction_secret_like_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        scratch_one_address: u64,
        scratch_two_address: u64,
        output_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_redaction_gateway_extra_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        scratch_address: u64,
        output_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn buffer_capacity(input_len: usize) -> Result<usize, MojoError> {
    input_len
        .checked_mul(3)
        .and_then(|value| value.checked_add(32))
        .map(|value| value.max(32))
        .ok_or(MojoError::Capacity)
}

fn output_string(output: Vec<u8>, written: i64) -> Result<String, MojoError> {
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonInspectionMode {
    SchemaOnly,
    DirectStrings,
    AllStrings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsonSensitiveKind {
    None,
    PrivateKey,
    ApiKey,
    AccessToken,
    Password,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonFieldPlan {
    pub inspection_mode: JsonInspectionMode,
    pub sensitive_kind: JsonSensitiveKind,
    pub known_protocol_metadata: bool,
    pub unsupported_modality: bool,
    pub tools_field: bool,
}

pub fn json_field_plan(value: &str) -> Result<JsonFieldPlan, MojoError> {
    let mut output = [-1_i64; 5];
    status(unsafe {
        prodex_redaction_json_field_plan_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let inspection_mode = match output[0] {
        0 => JsonInspectionMode::SchemaOnly,
        1 => JsonInspectionMode::DirectStrings,
        2 => JsonInspectionMode::AllStrings,
        _ => return Err(MojoError::InvalidOutput),
    };
    let sensitive_kind = match output[1] {
        0 => JsonSensitiveKind::None,
        1 => JsonSensitiveKind::PrivateKey,
        2 => JsonSensitiveKind::ApiKey,
        3 => JsonSensitiveKind::AccessToken,
        4 => JsonSensitiveKind::Password,
        _ => return Err(MojoError::InvalidOutput),
    };
    let bool_output = |value| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    Ok(JsonFieldPlan {
        inspection_mode,
        sensitive_kind,
        known_protocol_metadata: bool_output(output[2])?,
        unsupported_modality: bool_output(output[3])?,
        tools_field: bool_output(output[4])?,
    })
}

pub fn key_looks_sensitive(value: &str) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_redaction_key_sensitive_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn redact_secret_like_text(value: &str) -> Result<String, MojoError> {
    let capacity = buffer_capacity(value.len())?;
    let mut scratch_one = vec![0_u8; capacity];
    let mut scratch_two = vec![0_u8; capacity];
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let result = unsafe {
        prodex_redaction_secret_like_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            scratch_one.as_mut_ptr() as usize as u64,
            scratch_two.as_mut_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    status(result)?;
    output_string(output, written)
}

fn redact_gateway_extra(value: &str) -> Result<String, MojoError> {
    let capacity = buffer_capacity(value.len())?;
    let mut scratch = vec![0_u8; capacity];
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let result = unsafe {
        prodex_redaction_gateway_extra_v1(
            REDACTION_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            scratch.as_mut_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    status(result)?;
    output_string(output, written)
}

pub fn redact_gateway_text(value: &str) -> Result<String, MojoError> {
    let redacted = redact_secret_like_text(value)?;
    redact_gateway_extra(&redacted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_kernel_smoke() {
        assert!(key_looks_sensitive("X-API-Key").unwrap());
        assert_eq!(
            json_field_plan("arguments").unwrap(),
            JsonFieldPlan {
                inspection_mode: JsonInspectionMode::AllStrings,
                sensitive_kind: JsonSensitiveKind::None,
                known_protocol_metadata: false,
                unsupported_modality: false,
                tools_field: false,
            }
        );
        assert_eq!(
            json_field_plan("private_key").unwrap().sensitive_kind,
            JsonSensitiveKind::PrivateKey
        );
        assert!(json_field_plan("model").unwrap().known_protocol_metadata);
        assert!(json_field_plan("input_image").unwrap().unsupported_modality);
        assert!(json_field_plan("tools").unwrap().tools_field);
        assert_eq!(
            redact_secret_like_text("Authorization: Bearer synthetic-token").unwrap(),
            "Authorization: Bearer <redacted>"
        );
        assert_eq!(
            redact_gateway_text("user@example.test 4111-1111-1111-1111").unwrap(),
            "<redacted><redacted>"
        );
    }
}
