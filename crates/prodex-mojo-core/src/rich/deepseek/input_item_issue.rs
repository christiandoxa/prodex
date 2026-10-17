//! The compiled DeepSeek input-item issue-message owner, reused by Kiro.
//! Rust supplies provider-specific labels and validates output, not error text.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_deepseek_input_item_issue_message_v1(
        abi_version: i64,
        issue: u64,
        provider_address: u64,
        provider_length: i64,
        detail_address: u64,
        detail_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Render an issue emitted by the compiled DeepSeek input-item classifier.
/// Unknown positive issue tags intentionally preserve the generic diagnostic.
pub fn deepseek_input_item_issue_message(
    issue: u64,
    provider_label: &str,
    detail: &str,
) -> Result<String, MojoError> {
    let capacity = provider_label
        .len()
        .checked_add(detail.len())
        .and_then(|len| len.checked_add(256))
        .ok_or(MojoError::Capacity)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_deepseek_input_item_issue_message_v1(
            1,
            issue,
            provider_label.as_ptr() as usize as u64,
            i64::try_from(provider_label.len()).map_err(|_| MojoError::InvalidInput)?,
            detail.as_ptr() as usize as u64,
            i64::try_from(detail.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_item_issue_messages_match_every_existing_provider_contract() {
        let cases: &[(u64, &str)] = &[
            (1, " input items must be objects"),
            (
                2,
                " chat prefix completion requires the beta chat endpoint, which is outside this Responses adapter",
            ),
            (3, " message role must be a string"),
            (
                4,
                " message role `` is not supported by this Responses adapter",
            ),
            (5, " message content must be a string, object, or array"),
            (
                6,
                " per-message cache_control is not supported by this Responses adapter because DeepSeek context caching is automatic",
            ),
            (
                7,
                " text-only adapter does not support object message content parts without text or type",
            ),
            (
                8,
                " text-only adapter does not support message content part type ``",
            ),
            (9, " input item validation returned an unknown result"),
            (10, " input tool items require a call_id"),
            (11, " input tool call items require a function name"),
            (12, " input tool output items require output content"),
            (
                13,
                " local_shell_call action.command must be an array of strings",
            ),
            (14, " local_shell_call requires a command"),
            (
                15,
                " input item type `` is not supported by this Responses adapter",
            ),
            (16, " text content parts require a text field"),
            (17, " input_text content parts require a text field"),
            (18, " output_text content parts require a text field"),
            (19, " input_text content parts require a text field"),
            (20, " output_text content parts require a text field"),
            (21, " text content parts require a text field"),
            (22, " input item validation returned an unknown result"),
        ];
        for provider in ["DeepSeek", "Kiro", "Provider🔥"] {
            for &(tag, suffix) in cases {
                assert_eq!(
                    deepseek_input_item_issue_message(tag, provider, "").unwrap(),
                    format!("{provider}{suffix}"),
                    "{provider:?}/{tag}",
                );
            }
        }
        assert_eq!(
            deepseek_input_item_issue_message(4, "Kiro", "critic").unwrap(),
            "Kiro message role `critic` is not supported by this Responses adapter",
        );
        assert_eq!(
            deepseek_input_item_issue_message(8, "DeepSeek", "image_東京🧭").unwrap(),
            "DeepSeek text-only adapter does not support message content part type `image_東京🧭`",
        );
        assert_eq!(
            deepseek_input_item_issue_message(15, "Kiro", "future_type").unwrap(),
            "Kiro input item type `future_type` is not supported by this Responses adapter",
        );
    }

    #[test]
    fn input_item_issue_abi_rejects_invalid_inputs_before_writing() {
        let label = b"Kiro";
        let detail = b"critic";
        let label_ptr = label.as_ptr() as usize as u64;
        let detail_ptr = detail.as_ptr() as usize as u64;
        let mut buffer = [91_u8; 150];
        let mut written = 41_i64;
        let out = buffer.as_mut_ptr() as usize as u64;
        let count = (&mut written as *mut i64) as usize as u64;
        let call = |version,
                    issue,
                    provider_ptr,
                    provider_len,
                    detail_ptr,
                    detail_len,
                    output,
                    capacity,
                    written_ptr| unsafe {
            prodex_deepseek_input_item_issue_message_v1(
                version,
                issue,
                provider_ptr,
                provider_len,
                detail_ptr,
                detail_len,
                output,
                capacity,
                written_ptr,
            )
        };
        assert_eq!(call(2, 4, label_ptr, 4, detail_ptr, 6, out, 150, count), 4);
        assert_eq!(call(1, 0, label_ptr, 4, detail_ptr, 6, out, 150, count), 1);
        assert_eq!(call(1, 4, 0, 4, detail_ptr, 6, out, 150, count), 1);
        assert_eq!(call(1, 4, label_ptr, -1, detail_ptr, 6, out, 150, count), 1);
        assert_eq!(call(1, 4, label_ptr, 4, 0, 6, out, 150, count), 1);
        assert_eq!(call(1, 4, label_ptr, 4, detail_ptr, -1, out, 150, count), 1);
        assert_eq!(call(1, 4, label_ptr, 4, detail_ptr, 6, 0, 150, count), 1);
        assert_eq!(call(1, 4, label_ptr, 4, detail_ptr, 6, out, 150, 0), 1);
        assert_eq!(call(1, 4, label_ptr, 4, detail_ptr, 6, out, 1, count), 3);
        let bad_utf8 = [0xff_u8];
        assert_eq!(
            call(
                1,
                4,
                bad_utf8.as_ptr() as usize as u64,
                1,
                detail_ptr,
                6,
                out,
                150,
                count
            ),
            1
        );
        assert_eq!(buffer, [91; 150]);
        assert_eq!(written, 41);
        assert_eq!(
            deepseek_input_item_issue_message(0, "Kiro", ""),
            Err(MojoError::InvalidInput),
        );
    }
}
