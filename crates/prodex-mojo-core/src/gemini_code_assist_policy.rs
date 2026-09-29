use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_gemini_code_assist_endpoint_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;

    fn prodex_gemini_code_assist_tier_label_v1(
        abi_version: i64,
        id_address: u64,
        id_length: i64,
        id_present: i64,
        name_address: u64,
        name_length: i64,
        name_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;
}

fn signed(value: usize) -> Result<i64, MojoError> {
    i64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

fn status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_optional_string(
    output: Vec<u8>,
    written: i64,
    present: i64,
) -> Result<Option<String>, MojoError> {
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    match present {
        0 if written == 0 => Ok(None),
        1 => String::from_utf8(output[..written].to_vec())
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn normalize_gemini_code_assist_endpoint(input: &str) -> Result<Option<String>, MojoError> {
    let mut output = vec![0_u8; input.len().max(1)];
    let mut written = 0_i64;
    let mut present = 0_i64;
    status(unsafe {
        prodex_gemini_code_assist_endpoint_v1(
            ABI_VERSION,
            input.as_ptr() as usize as u64,
            signed(input.len())?,
            output.as_mut_ptr() as usize as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    })?;
    decode_optional_string(output, written, present)
}

pub fn gemini_code_assist_tier_label(
    id: Option<&str>,
    name: Option<&str>,
) -> Result<Option<String>, MojoError> {
    let id_value = id.unwrap_or_default();
    let name_value = name.unwrap_or_default();
    let capacity = id_value
        .len()
        .max(name_value.len())
        .max("standard".len())
        .max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let mut present = 0_i64;
    status(unsafe {
        prodex_gemini_code_assist_tier_label_v1(
            ABI_VERSION,
            id_value.as_ptr() as usize as u64,
            signed(id_value.len())?,
            i64::from(id.is_some()),
            name_value.as_ptr() as usize as u64,
            signed(name_value.len())?,
            i64::from(name.is_some()),
            output.as_mut_ptr() as usize as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    })?;
    decode_optional_string(output, written, present)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_assist_policy_preserves_endpoint_normalization() {
        assert_eq!(
            normalize_gemini_code_assist_endpoint(" https://example.test/v1/// ").unwrap(),
            Some("https://example.test/v1".to_string())
        );
        assert_eq!(
            normalize_gemini_code_assist_endpoint(" /// ").unwrap(),
            None
        );
        assert_eq!(normalize_gemini_code_assist_endpoint(" \t ").unwrap(), None);
    }

    #[test]
    fn code_assist_policy_preserves_tier_label_semantics() {
        assert_eq!(
            gemini_code_assist_tier_label(Some("free-tier"), None).unwrap(),
            Some("free".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(Some(" Custom-tier "), Some("ignored")).unwrap(),
            Some("custom".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(Some(" FREE-TIER "), None).unwrap(),
            Some("free-tier".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(Some("-tier"), None).unwrap(),
            Some(String::new())
        );
        assert_eq!(
            gemini_code_assist_tier_label(Some("  "), Some("Google One AI ULTRA plan")).unwrap(),
            Some("ultra".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(None, Some(" Standard workspace ")).unwrap(),
            Some("standard".to_string())
        );
        assert_eq!(
            gemini_code_assist_tier_label(None, Some("Custom Plan")).unwrap(),
            Some("Custom Plan".to_string())
        );
        assert_eq!(gemini_code_assist_tier_label(None, None).unwrap(), None);
    }
}
