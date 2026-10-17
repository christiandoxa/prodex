//! The compiled chat-tool name decoder owns reversible namespace precedence.
//! Rust validates borrowed UTF-8 spans, not separator or trimming semantics.

use crate::MojoError;

unsafe extern "C" {
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

#[cfg(test)]
mod tests {
    use super::*;

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
