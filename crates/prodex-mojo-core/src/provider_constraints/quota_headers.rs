//! Checked production quotas-to-Codex header text projection.
//! Rust owns borrowed inputs and output storage, not Unicode or model precedence.

use crate::MojoError;

mod duration;
pub use duration::provider_quota_duration_seconds;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_provider_quota_reset_epoch_v1(
        abi_version: i64,
        operation: i64,
        number: i64,
        now: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_provider_quota_percent_v1(
        abi_version: i64,
        operation: i64,
        first_present: i64,
        first: f64,
        second_present: i64,
        second: f64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;
    fn prodex_provider_quota_limit_id_v1(
        abi_version: i64,
        lowered_address: u64,
        lowered_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
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

#[repr(i64)]
#[derive(Clone, Copy)]
enum QuotaResetSource {
    Numeric = 0,
    Duration = 1,
}

fn reset_epoch(source: QuotaResetSource, number: i64, now: i64) -> Result<i64, MojoError> {
    let mut result = i64::MIN;
    status(unsafe {
        prodex_provider_quota_reset_epoch_v1(
            ABI_VERSION,
            source as i64,
            number,
            now,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    Ok(result)
}

/// Mojo owns numeric quota reset classification between milliseconds,
/// absolute seconds and nonnegative relative seconds.
pub fn provider_quota_numeric_reset_epoch(number: i64, now: i64) -> Result<i64, MojoError> {
    reset_epoch(QuotaResetSource::Numeric, number, now)
}

/// Mojo applies the saturated relative duration after wall-clock acquisition.
pub fn provider_quota_duration_reset_epoch(seconds: i64, now: i64) -> Result<i64, MojoError> {
    reset_epoch(QuotaResetSource::Duration, seconds, now)
}

#[repr(i64)]
#[derive(Clone, Copy)]
enum QuotaPercentSource {
    Gemini = 0,
    OpenAi = 1,
}

fn quota_percent(
    source: QuotaPercentSource,
    first: Option<f64>,
    second: Option<f64>,
) -> Result<Option<String>, MojoError> {
    let mut buffer = [0_u8; 5];
    let mut written = -1_i64;
    let mut present = -1_i64;
    status(unsafe {
        prodex_provider_quota_percent_v1(
            ABI_VERSION,
            source as i64,
            i64::from(first.is_some()),
            first.unwrap_or(0.0),
            i64::from(second.is_some()),
            second.unwrap_or(0.0),
            buffer.as_mut_ptr() as usize as u64,
            i64::try_from(buffer.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    })?;
    match (present, written) {
        (0, 0) => Ok(None),
        (1, count) if count > 0 => result_string(buffer.to_vec(), count).map(Some),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Convert Gemini's remaining fraction to a clamped, one-decimal Codex quota
/// used-percent field, including the historical NaN/infinity behavior.
pub fn provider_quota_gemini_used_percent(
    remaining_fraction: Option<f64>,
) -> Result<Option<String>, MojoError> {
    quota_percent(QuotaPercentSource::Gemini, remaining_fraction, None)
}

/// Convert OpenAI-compatible limit and remaining headers to a Codex used
/// percentage. Missing/invalid limits are skipped in compiled Mojo.
pub fn provider_quota_openai_used_percent(
    limit: Option<f64>,
    remaining: Option<f64>,
) -> Result<Option<String>, MojoError> {
    quota_percent(QuotaPercentSource::OpenAi, limit, remaining)
}

/// Preserve Rust's Unicode contextual lowercase service while giving the
/// compiled Mojo owner exclusive authority over allowed ASCII characters,
/// separator collapsing and the final hyphenated Codex quota header ID.
pub fn provider_quota_limit_id(value: &str) -> Result<String, MojoError> {
    let lowered = value.trim().to_lowercase();
    let mut buffer = vec![0_u8; lowered.len().max(1)];
    let mut written = -1_i64;
    status(unsafe {
        prodex_provider_quota_limit_id_v1(
            ABI_VERSION,
            lowered.as_ptr() as usize as u64,
            len(&lowered)?,
            buffer.as_mut_ptr() as usize as u64,
            i64::try_from(buffer.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    result_string(buffer, written)
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
    fn quota_reset_numeric_epoch_classification_and_saturation_is_compiled_mojo() {
        for (number, now, expected) in [
            (50, 1000, 1050),
            (940, 1000, 1940),
            (941, 1000, 941),
            (0, 1000, 1000),
            (-99, 1000, 1000),
            (10_000_000_000, 1000, 10_000_000_000),
            (10_000_000_001, 1000, 10_000_000),
            (i64::MAX, 1000, i64::MAX / 1000),
            (i64::MIN, i64::MIN, i64::MIN),
            (-1, i64::MIN, -1),
        ] {
            assert_eq!(
                provider_quota_numeric_reset_epoch(number, now).unwrap(),
                expected,
                "{number}/{now}"
            );
        }
        for (duration, now, expected) in [
            (0, 100, 100),
            (12, 100, 112),
            (15, i64::MAX - 5, i64::MAX),
            (i64::MAX, 100, i64::MAX),
            (1, i64::MIN, i64::MIN + 1),
        ] {
            assert_eq!(
                provider_quota_duration_reset_epoch(duration, now).unwrap(),
                expected,
                "{duration}/{now}"
            );
        }
    }

    #[test]
    fn quota_reset_epoch_abi_rejects_invalid_modes_without_mutation() {
        let mut output = 123_i64;
        let destination = (&mut output as *mut i64) as usize as u64;
        unsafe {
            assert_eq!(
                prodex_provider_quota_reset_epoch_v1(2, 0, 30, 100, destination),
                4
            );
            assert_eq!(
                prodex_provider_quota_reset_epoch_v1(1, 2, 30, 100, destination),
                1
            );
            assert_eq!(
                prodex_provider_quota_reset_epoch_v1(1, -1, 30, 100, destination),
                1
            );
            assert_eq!(
                prodex_provider_quota_reset_epoch_v1(1, 1, -1, 100, destination),
                1
            );
            assert_eq!(prodex_provider_quota_reset_epoch_v1(1, 1, 30, 100, 0), 1);
        }
        assert_eq!(output, 123);
    }

    #[test]
    fn quota_limit_id_mojo_owns_ascii_projection_and_separator_collapse() {
        for (input, expected) in [
            ("", ""),
            ("_---_", ""),
            (" Gemini_Responses  ", "gemini-responses"),
            ("GEMINI____TOKEN", "gemini-token"),
            ("A--b__c", "a-b-c"),
            ("a 🧭__b", "a-b"),
            ("  --a--b--  ", "a-b"),
            ("K-İ-ABC", "k-i-abc"),
            ("Σ", ""),
            ("-1-2--", "1-2"),
            ("aé_b", "a-b"),
            ("a\nb", "ab"),
        ] {
            assert_eq!(
                provider_quota_limit_id(input).unwrap(),
                expected,
                "{input:?}"
            );
        }
    }

    #[test]
    fn quota_limit_id_abi_rejects_malformed_and_undersized_buffers() {
        let value = b"abc";
        let input = value.as_ptr() as usize as u64;
        let mut output = [99_u8; 8];
        let mut written = 17_i64;
        let out = output.as_mut_ptr() as usize as u64;
        let w = (&mut written as *mut i64) as usize as u64;
        unsafe {
            assert_eq!(prodex_provider_quota_limit_id_v1(2, input, 3, out, 8, w), 4);
            assert_eq!(prodex_provider_quota_limit_id_v1(1, 0, 3, out, 8, w), 1);
            assert_eq!(
                prodex_provider_quota_limit_id_v1(1, input, -1, out, 8, w),
                1
            );
            assert_eq!(prodex_provider_quota_limit_id_v1(1, input, 3, out, 2, w), 1);
            assert_eq!(prodex_provider_quota_limit_id_v1(1, input, 3, out, 8, 0), 1);
        }
        assert_eq!(output, [99; 8]);
        assert_eq!(written, 17);
    }

    #[test]
    fn used_percent_from_gemini_and_openai_is_compiled_mojo_owned() {
        for (remaining, expected) in [
            (None, None),
            (Some(1.0), Some("0")),
            (Some(0.0), Some("100")),
            (Some(0.25), Some("75")),
            (Some(0.125), Some("87.5")),
            (Some(0.333), Some("66.7")),
            (Some(1.1), Some("0")),
            (Some(-0.15), Some("100")),
            (Some(f64::NAN), Some("NaN")),
            (Some(f64::INFINITY), Some("0")),
            (Some(f64::NEG_INFINITY), Some("100")),
        ] {
            assert_eq!(
                provider_quota_gemini_used_percent(remaining)
                    .unwrap()
                    .as_deref(),
                expected,
                "remaining={remaining:?}"
            );
        }
        for (limit, remaining, expected) in [
            (None, Some(2.0), None),
            (Some(2.0), None, None),
            (Some(0.0), Some(0.0), None),
            (Some(-1.0), Some(0.0), None),
            (Some(100.0), Some(75.0), Some("25")),
            (Some(100.0), Some(0.0), Some("100")),
            (Some(100.0), Some(200.0), Some("0")),
            (Some(3.0), Some(1.0), Some("66.7")),
            (Some(1.0), Some(-10.0), Some("100")),
            (Some(f64::INFINITY), Some(1.0), None),
            (Some(f64::NAN), Some(1.0), None),
            (Some(1.0), Some(f64::NAN), None),
        ] {
            assert_eq!(
                provider_quota_openai_used_percent(limit, remaining)
                    .unwrap()
                    .as_deref(),
                expected,
                "{limit:?}/{remaining:?}"
            );
        }
    }

    #[test]
    fn strict_quota_percent_abi_validates_mode_presence_and_capacity_before_output() {
        let mut buffer = [88_u8; 8];
        let mut written = 19_i64;
        let mut present = 71_i64;
        let out = buffer.as_mut_ptr() as usize as u64;
        let w = (&mut written as *mut i64) as usize as u64;
        let p = (&mut present as *mut i64) as usize as u64;
        unsafe {
            assert_eq!(
                prodex_provider_quota_percent_v1(2, 0, 1, 0.25, 0, 0.0, out, 8, w, p),
                4
            );
            assert_eq!(
                prodex_provider_quota_percent_v1(1, 7, 1, 0.25, 0, 0.0, out, 8, w, p),
                1
            );
            assert_eq!(
                prodex_provider_quota_percent_v1(1, 0, 2, 0.25, 0, 0.0, out, 8, w, p),
                1
            );
            assert_eq!(
                prodex_provider_quota_percent_v1(1, 0, 1, 0.25, 1, 0.0, out, 8, w, p),
                1
            );
            assert_eq!(
                prodex_provider_quota_percent_v1(1, 0, 1, 0.25, 0, 0.0, out, 4, w, p),
                1
            );
            assert_eq!(
                prodex_provider_quota_percent_v1(1, 0, 1, 0.25, 0, 0.0, 0, 8, w, p),
                1
            );
            assert_eq!(
                prodex_provider_quota_percent_v1(1, 0, 1, 0.25, 0, 0.0, out, 8, 0, p),
                1
            );
            assert_eq!(
                prodex_provider_quota_percent_v1(1, 0, 1, 0.25, 0, 0.0, out, 8, w, 0),
                1
            );
        }
        assert_eq!(buffer, [88; 8]);
        assert_eq!(written, 19);
        assert_eq!(present, 71);
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
