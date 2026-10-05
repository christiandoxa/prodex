use super::{
    RichStringView, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view,
};
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const FIELD_COUNT: usize = 12;
const PRESENT_ITEM_TYPE: i64 = 1 << 0;
const PRESENT_ROLE: i64 = 1 << 1;
const PRESENT_NAME: i64 = 1 << 2;
const PRESENT_CALL_ID: i64 = 1 << 3;
const PRESENT_CONTENT: i64 = 1 << 4;

unsafe extern "C" {
    fn prodex_mojo_gemini_compact_snippet_v1(
        abi_version: i64,
        views_address: u64,
        view_count: i64,
        presence: i64,
        maximum: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        emitted_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_compact_truncate_v1(
        abi_version: i64,
        mode: i64,
        input_address: u64,
        input_length: i64,
        maximum: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_gemini_compact_local_summary_v1(
        abi_version: i64,
        model_address: u64,
        snippets_address: u64,
        snippet_count: i64,
        original_input_count: i64,
        maximum: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GeminiCompactSnippetInput<'a> {
    pub item_type: Option<&'a str>,
    pub role: Option<&'a str>,
    pub name: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub content: Option<&'a str>,
    pub text: Option<&'a str>,
    pub arguments: Option<&'a str>,
    pub tool_input: Option<&'a str>,
    pub tool_output: Option<&'a str>,
    pub action: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub generic: Option<&'a str>,
}

fn optional_view(value: Option<&str>) -> RichStringView {
    value.map(view).unwrap_or_default()
}

pub fn format_gemini_compact_snippet(
    input: GeminiCompactSnippetInput<'_>,
    max_bytes: usize,
) -> Result<Option<String>, MojoError> {
    ensure_rich_abi()?;
    let fields = [
        optional_view(input.item_type),
        optional_view(input.role),
        optional_view(input.name),
        optional_view(input.call_id),
        optional_view(input.content),
        optional_view(input.text),
        optional_view(input.arguments),
        optional_view(input.tool_input),
        optional_view(input.tool_output),
        optional_view(input.action),
        optional_view(input.summary),
        optional_view(input.generic),
    ];
    let mut presence = 0_i64;
    if input.item_type.is_some() {
        presence |= PRESENT_ITEM_TYPE;
    }
    if input.role.is_some() {
        presence |= PRESENT_ROLE;
    }
    if input.name.is_some() {
        presence |= PRESENT_NAME;
    }
    if input.call_id.is_some() {
        presence |= PRESENT_CALL_ID;
    }
    if input.content.is_some() {
        presence |= PRESENT_CONTENT;
    }
    let output_capacity = max_bytes.checked_add(4).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; output_capacity.max(4)];
    let mut written = 0_i64;
    let mut emitted = 0_i64;
    let status = unsafe {
        prodex_mojo_gemini_compact_snippet_v1(
            ABI_VERSION,
            mojo_pointer_address(fields.as_ptr()),
            i64::try_from(FIELD_COUNT).map_err(|_| MojoError::InvalidInput)?,
            presence,
            i64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
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
    if written > max_bytes || written > output.len() {
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

pub fn format_gemini_local_compact_summary(
    model: Option<&str>,
    snippets: &[String],
    original_input_count: usize,
    max_bytes: usize,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let model = optional_view(model);
    let snippet_views = snippets
        .iter()
        .map(|snippet| view(snippet))
        .collect::<Vec<_>>();
    let output_capacity = max_bytes.checked_add(4).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; output_capacity.max(4)];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_gemini_compact_local_summary_v1(
            ABI_VERSION,
            mojo_pointer_address(&model),
            mojo_pointer_address(snippet_views.as_ptr()),
            i64::try_from(snippet_views.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(original_input_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
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
    if written > max_bytes || written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

fn truncate_gemini_compact_utf8_mode(
    text: &str,
    max_bytes: usize,
    mode: i64,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let mut output = vec![0_u8; max_bytes.max(1)];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_gemini_compact_truncate_v1(
            ABI_VERSION,
            mode,
            mojo_pointer_address(text.as_ptr()),
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
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
    if written > max_bytes || written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn truncate_gemini_compact_utf8(text: &str, max_bytes: usize) -> Result<String, MojoError> {
    truncate_gemini_compact_utf8_mode(text, max_bytes, 1)
}

pub fn truncate_gemini_compact_utf8_edges(
    text: &str,
    max_bytes: usize,
) -> Result<String, MojoError> {
    truncate_gemini_compact_utf8_mode(text, max_bytes, 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_compact_snippet_formatting_is_mojo_owned() {
        assert_eq!(
            format_gemini_compact_snippet(
                GeminiCompactSnippetInput {
                    item_type: Some("message"),
                    role: Some("user"),
                    content: Some("hello"),
                    ..Default::default()
                },
                768,
            )
            .unwrap()
            .as_deref(),
            Some("user message: hello")
        );
        assert_eq!(
            format_gemini_compact_snippet(
                GeminiCompactSnippetInput {
                    item_type: Some("message"),
                    content: Some("   "),
                    text: Some("ignored fallback"),
                    ..Default::default()
                },
                768,
            )
            .unwrap()
            .as_deref(),
            Some("unknown message with no text content")
        );
        assert_eq!(
            format_gemini_compact_snippet(
                GeminiCompactSnippetInput {
                    item_type: Some("function_call"),
                    arguments: Some("{}"),
                    ..Default::default()
                },
                768,
            )
            .unwrap()
            .as_deref(),
            Some("tool call function (unknown): {}")
        );
        assert_eq!(
            format_gemini_compact_snippet(
                GeminiCompactSnippetInput {
                    item_type: Some("reasoning"),
                    summary: Some(" \t"),
                    ..Default::default()
                },
                768,
            )
            .unwrap(),
            None
        );
        assert_eq!(
            format_gemini_compact_snippet(
                GeminiCompactSnippetInput {
                    item_type: Some("unknown_kind"),
                    generic: Some("value"),
                    ..Default::default()
                },
                768,
            )
            .unwrap()
            .as_deref(),
            Some("unknown_kind: value")
        );
        let long = "月".repeat(1_000);
        let snippet = format_gemini_compact_snippet(
            GeminiCompactSnippetInput {
                item_type: Some("web_search_call"),
                action: Some(&long),
                ..Default::default()
            },
            64,
        )
        .unwrap()
        .unwrap();
        assert!(snippet.len() <= 64);
        assert!(snippet.ends_with("\n[truncated]"));
        assert!(std::str::from_utf8(snippet.as_bytes()).is_ok());
    }

    #[test]
    fn gemini_local_compact_summary_formatting_is_mojo_owned() {
        let snippets = (0..26)
            .map(|index| format!("snippet-{index}\nnext"))
            .collect::<Vec<_>>();
        let summary =
            format_gemini_local_compact_summary(Some("  gemini-test  "), &snippets, 30, 24 * 1024)
                .unwrap();
        assert!(summary.starts_with("Local Prodex compact fallback summary.\n\nModel: gemini-test\nOriginal input items: 30\nRetained recent items: 24\n\nRecent conversation and tool state:\n"));
        assert!(!summary.contains("- snippet-0\n"));
        assert!(!summary.contains("- snippet-1\n"));
        assert!(summary.contains("- snippet-2\n  next\n"));
        assert!(summary.contains("- snippet-25\n  next\n"));
        let empty = format_gemini_local_compact_summary(Some("   "), &[], 0, 24 * 1024).unwrap();
        assert!(empty.contains("Model: unknown"));
        assert!(empty.contains("- No parseable recent message or tool content was found."));
    }

    #[test]
    fn gemini_compact_utf8_truncation_is_mojo_owned() {
        let text = "月".repeat(100);
        for maximum in [0, 1, 12, 64, 512] {
            let tail = truncate_gemini_compact_utf8(&text, maximum).unwrap();
            let edges = truncate_gemini_compact_utf8_edges(&text, maximum).unwrap();
            assert!(tail.len() <= maximum);
            assert!(edges.len() <= maximum);
            assert!(std::str::from_utf8(tail.as_bytes()).is_ok());
            assert!(std::str::from_utf8(edges.as_bytes()).is_ok());
        }
        assert_eq!(truncate_gemini_compact_utf8("abcdef", 6).unwrap(), "abcdef");
        assert_eq!(
            truncate_gemini_compact_utf8_edges("abcdef", 6).unwrap(),
            "abcdef"
        );
    }
}
