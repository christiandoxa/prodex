//! The compiled chat-tool name decoder owns reversible namespace precedence.
//! Rust validates borrowed UTF-8 spans, not separator or trimming semantics.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_chat_compatible_tool_call_kind_v1(
        abi_version: i64,
        function_present: i64,
        name_present: i64,
        name_address: u64,
        name_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_chat_tool_namespace_split_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

/// Return two borrowed name spans when Mojo finds an explicit or MCP
/// namespace. None leaves the caller's original name untouched.
pub fn split_flat_namespace_tool_name(name: &str) -> Result<Option<(&str, &str)>, MojoError> {
    let mut result = [-1_i64; 3];
    let status = unsafe {
        prodex_chat_tool_namespace_split_v1(
            1,
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
            result.len() as i64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match result {
        [0, 0, 0] => Ok(None),
        [1, namespace_end, tool_start] => {
            let namespace_end =
                usize::try_from(namespace_end).map_err(|_| MojoError::InvalidOutput)?;
            let tool_start = usize::try_from(tool_start).map_err(|_| MojoError::InvalidOutput)?;
            if namespace_end == 0
                || namespace_end.checked_add(2) != Some(tool_start)
                || tool_start >= name.len()
            {
                return Err(MojoError::InvalidOutput);
            }
            let namespace = name.get(..namespace_end).ok_or(MojoError::InvalidOutput)?;
            let tool = name.get(tool_start..).ok_or(MojoError::InvalidOutput)?;
            Ok(Some((namespace, tool)))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatCompatibleToolCallKind {
    InvalidFunction,
    InvalidName,
    ToolSearch,
    ApplyPatch,
    Function,
}

/// Mojo owns chat-compatible tool-call name validity and special-tool
/// classification. The Rust host still parses JSON and materializes tool
/// response plans, but must not duplicate the classifier on ABI failures.
pub fn chat_compatible_tool_call_kind(
    function_present: bool,
    name: Option<&str>,
) -> Result<ChatCompatibleToolCallKind, MojoError> {
    let name_value = name.unwrap_or("");
    let mut result = -1_i64;
    let status = unsafe {
        prodex_chat_compatible_tool_call_kind_v1(
            1,
            i64::from(function_present),
            i64::from(name.is_some()),
            name_value.as_ptr() as usize as u64,
            i64::try_from(name_value.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut result as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match result {
        0 => Ok(ChatCompatibleToolCallKind::InvalidFunction),
        1 => Ok(ChatCompatibleToolCallKind::InvalidName),
        2 => Ok(ChatCompatibleToolCallKind::ToolSearch),
        3 => Ok(ChatCompatibleToolCallKind::ApplyPatch),
        4 => Ok(ChatCompatibleToolCallKind::Function),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_chat_compatible_tool_call_kind_preserves_special_names_and_unicode_validation() {
        use ChatCompatibleToolCallKind::*;
        for (present, name, expected) in [
            (false, None, InvalidFunction),
            (false, Some("tool_search"), InvalidFunction),
            (true, None, InvalidName),
            (true, Some(""), InvalidName),
            (true, Some("  \t\u{3000}"), InvalidName),
            (true, Some("tool_search"), ToolSearch),
            (true, Some("apply_patch"), ApplyPatch),
            (true, Some("tool_search "), Function),
            (true, Some("Tool_Search"), Function),
            (true, Some("apply_patch\u{2003}"), Function),
            (true, Some("functions__读取🔥"), Function),
            (true, Some("shell"), Function),
        ] {
            assert_eq!(
                chat_compatible_tool_call_kind(present, name).unwrap(),
                expected,
                "present={present} name={name:?}",
            );
        }
    }

    #[test]
    fn compiled_chat_compatible_tool_call_kind_abi_rejects_invalid_flags_utf8_and_pointers() {
        let name = b"tool_search";
        let source = name.as_ptr() as usize as u64;
        let mut result = 99_i64;
        let dest = (&mut result as *mut i64) as usize as u64;
        let invoke = |version, function, present, address, length, output| unsafe {
            prodex_chat_compatible_tool_call_kind_v1(
                version, function, present, address, length, output,
            )
        };
        assert_eq!(invoke(2, 1, 1, source, 11, dest), 4);
        assert_eq!(invoke(1, 2, 1, source, 11, dest), 1);
        assert_eq!(invoke(1, 1, 2, source, 11, dest), 1);
        assert_eq!(invoke(1, 1, 1, 0, 11, dest), 1);
        assert_eq!(invoke(1, 1, 1, source, -1, dest), 1);
        assert_eq!(invoke(1, 1, 1, source, 11, 0), 1);
        let invalid = [0xff_u8];
        assert_eq!(
            invoke(1, 1, 1, invalid.as_ptr() as usize as u64, 1, dest),
            1,
        );
        assert_eq!(result, 99);
    }

    #[test]
    fn compiled_chat_namespace_decoder_preserves_delimiter_precedence_and_unicode() {
        for (name, expected) in [
            ("", None),
            ("shell", None),
            ("shell--exec", Some(("shell", "exec"))),
            ("a--b--c", Some(("a--b", "c"))),
            ("--leading", None),
            ("trailing--", None),
            (" -- x", None),
            (
                "mcp__prodex_sqz__sqz_read_file",
                Some(("mcp__prodex_sqz", "sqz_read_file")),
            ),
            ("mcp____foo", Some(("mcp__", "foo"))),
            ("mcp__prodex__foo--bar", Some(("mcp__prodex__foo", "bar"))),
            ("mcp__prodex__foo--", Some(("mcp__prodex", "foo--"))),
            ("mcp__prodex__ ", None),
            ("mcp__repo__東京🔥", Some(("mcp__repo", "東京🔥"))),
            ("日本語--🔥", Some(("日本語", "🔥"))),
            ("mcp__repo--_tool", Some(("mcp__repo", "_tool"))),
        ] {
            assert_eq!(
                split_flat_namespace_tool_name(name).unwrap(),
                expected,
                "{name:?}"
            );
        }
    }

    #[test]
    fn compiled_chat_namespace_abi_rejects_invalid_utf8_and_shape_before_writes() {
        let name = b"shell--exec";
        let address = name.as_ptr() as usize as u64;
        let mut result = [87_i64; 3];
        let target = result.as_mut_ptr() as usize as u64;
        let invalid = [0xff_u8];
        let call = |version, addr, len, out, capacity| unsafe {
            prodex_chat_tool_namespace_split_v1(version, addr, len, out, capacity)
        };
        assert_eq!(call(2, address, name.len() as i64, target, 3), 4);
        assert_eq!(call(1, 0, name.len() as i64, target, 3), 1);
        assert_eq!(call(1, address, -1, target, 3), 1);
        assert_eq!(call(1, address, name.len() as i64, target, 2), 1);
        assert_eq!(call(1, address, name.len() as i64, 0, 3), 1);
        assert_eq!(call(1, invalid.as_ptr() as usize as u64, 1, target, 3), 1);
        assert_eq!(result, [87; 3]);
    }
}
