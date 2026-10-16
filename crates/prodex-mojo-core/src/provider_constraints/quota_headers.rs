//! Checked production quotas-to-Codex header text projection.
//! Rust owns borrowed inputs and output storage, not Unicode or model precedence.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_provider_quota_header_clean_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_provider_quota_model_label_v1(
        abi_version: i64,
        model_address: u64,
        model_length: i64,
        model_present: i64,
        token_address: u64,
        token_length: i64,
        token_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;
}

fn status(code: i64) -> Result<(), MojoError> {
    match code {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn result_string(bytes: Vec<u8>, written: i64) -> Result<String, MojoError> {
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > bytes.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(bytes[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

/// Remove CR/LF and Unicode edge whitespace using compiled Mojo.
pub fn provider_quota_clean_header(value: &str) -> Result<String, MojoError> {
    let mut buffer = vec![0_u8; value.len().max(1)];
    let mut written = -1_i64;
    status(unsafe {
        prodex_provider_quota_header_clean_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            len(value)?,
            buffer.as_mut_ptr() as usize as u64,
            i64::try_from(buffer.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    result_string(buffer, written)
}

/// Model ID wins over token type when trimmed/nonempty, even if the
/// stripped models/ prefix subsequently produces empty output.
pub fn provider_quota_model_label(
    model_id: Option<&str>,
    token_type: Option<&str>,
) -> Result<Option<String>, MojoError> {
    let model = model_id.unwrap_or("");
    let token = token_type.unwrap_or("");
    let capacity = model.len().max(token.len()).max(1);
    let mut buffer = vec![0_u8; capacity];
    let mut written = -1_i64;
    let mut present = -1_i64;
    status(unsafe {
        prodex_provider_quota_model_label_v1(
            ABI_VERSION,
            model.as_ptr() as usize as u64,
            len(model)?,
            i64::from(model_id.is_some()),
            token.as_ptr() as usize as u64,
            len(token)?,
            i64::from(token_type.is_some()),
            buffer.as_mut_ptr() as usize as u64,
            i64::try_from(buffer.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    })?;
    match (present, written) {
        (0, 0) => Ok(None),
        (1, amount) if amount > 0 => result_string(buffer, amount).map(Some),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleaned_provider_headers_remove_crlf_and_unicode_edge_space() {
        for (text, expected) in [
            ("", ""),
            ("\r\n", ""),
            ("  hello  ", "hello"),
            ("\u{2003}東京\u{3000}", "東京"),
            ("first\r\nsecond", "firstsecond"),
            ("  first\r\n second  ", "first second"),
            ("  a\t b  ", "a\t b"),
            ("\u{200b}", "\u{200b}"),
            ("🌐\r\n東京", "🌐東京"),
        ] {
            assert_eq!(
                provider_quota_clean_header(text).unwrap(),
                expected,
                "{text:?}"
            );
        }
    }

    #[test]
    fn gemini_model_label_owns_source_precedence_prefix_and_unicode() {
        for (model, token, expected) in [
            (None, None, None),
            (None, Some(""), None),
            (Some(" "), Some("output"), Some("output")),
            (
                Some(" models/gemini-2.5-pro "),
                Some("irrelevant"),
                Some("gemini-2.5-pro"),
            ),
            (Some("models/"), Some("fallback"), None),
            (Some("models/🌐"), Some("ignored"), Some("🌐")),
            (
                Some("models\t/gemini"),
                Some("ignored"),
                Some("models\t/gemini"),
            ),
            (Some("\r\n\u{2003}models/東京\u{3000}"), None, Some("東京")),
            (None, Some("  input\r\n extra  "), Some("input extra")),
            (
                Some("MODELS/gemini"),
                Some("ignored"),
                Some("MODELS/gemini"),
            ),
        ] {
            assert_eq!(
                provider_quota_model_label(model, token).unwrap().as_deref(),
                expected,
                "{model:?}/{token:?}"
            );
        }
    }

    #[test]
    fn strict_quota_header_abi_rejects_invalid_buffers_without_writes() {
        let raw = b" a\rb ";
        let address = raw.as_ptr() as usize as u64;
        let mut buffer = [77_u8; 16];
        let mut written = 99_i64;
        let mut present = 55_i64;
        let out = buffer.as_mut_ptr() as usize as u64;
        let w = (&mut written as *mut i64) as usize as u64;
        let p = (&mut present as *mut i64) as usize as u64;
        unsafe {
            assert_eq!(
                prodex_provider_quota_header_clean_v1(2, address, 5, out, 16, w),
                4
            );
            assert_eq!(
                prodex_provider_quota_header_clean_v1(1, address, 5, out, 4, w),
                1
            );
            assert_eq!(
                prodex_provider_quota_header_clean_v1(1, 0, 5, out, 16, w),
                1
            );
            assert_eq!(
                prodex_provider_quota_header_clean_v1(1, address, 5, out, 16, 0),
                1
            );
            assert_eq!(
                prodex_provider_quota_model_label_v1(
                    2, address, 5, 1, address, 5, 1, out, 16, w, p
                ),
                4
            );
            assert_eq!(
                prodex_provider_quota_model_label_v1(
                    1, address, 5, 2, address, 5, 1, out, 16, w, p
                ),
                1
            );
            assert_eq!(
                prodex_provider_quota_model_label_v1(1, address, 5, 1, address, 5, 1, out, 4, w, p),
                1
            );
        }
        assert_eq!(buffer, [77; 16]);
        assert_eq!(written, 99);
        assert_eq!(present, 55);
    }
}
