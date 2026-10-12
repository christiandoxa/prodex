use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address};
use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_mojo_gemini_compact_text_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        emitted_address: u64,
    ) -> i64;
}

/// Extracts the compact text represented by one serialized JSON value.
pub fn extract_gemini_compact_text(raw: &str) -> Result<Option<String>, MojoError> {
    ensure_rich_abi()?;
    let output_capacity = raw.len().checked_add(1).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; output_capacity.max(1)];
    let mut written = 0_i64;
    let mut emitted = 0_i64;
    let status = unsafe {
        prodex_mojo_gemini_compact_text_v1(
            ABI_VERSION,
            mojo_pointer_address(raw.as_ptr()),
            i64::try_from(raw.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
            mojo_mut_pointer_address(&mut emitted),
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::InvalidOutput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    match emitted {
        0 if written == 0 => Ok(None),
        1 => String::from_utf8(output[..written].to_vec())
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_is_mojo_owned_and_decodes_nested_json() {
        assert_eq!(
            extract_gemini_compact_text(r#"{"content":[{"text":"hello"}]}"#).unwrap(),
            Some("hello".to_string())
        );
        assert_eq!(
            extract_gemini_compact_text(r#"{"wrapper":{"query":"nested"}}"#).unwrap(),
            Some("nested".to_string())
        );
        assert_eq!(
            extract_gemini_compact_text(r#"{"query":"first","text":"second"}"#).unwrap(),
            Some("second".to_string())
        );
        assert_eq!(
            extract_gemini_compact_text(r#"["a",{"output":"b"},true]"#).unwrap(),
            Some("a\nb\ntrue".to_string())
        );
        assert_eq!(
            extract_gemini_compact_text(r#"{"text":"\u6708\nnext"}"#).unwrap(),
            Some("月\nnext".to_string())
        );
        assert_eq!(
            extract_gemini_compact_text(r#"{"text":"  "}"#).unwrap(),
            None
        );
        assert_eq!(extract_gemini_compact_text("null").unwrap(), None);
    }
}
