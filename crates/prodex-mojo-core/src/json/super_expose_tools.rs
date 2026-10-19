//! Compiled MCP catalog rendering over non-secret optional-tool identifiers.
use super::{JsonNode, signed, transform_json};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_super_expose_tool_catalog_v1(
        abi: i64,
        max_event_page: i64,
        exec_only: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
}

/// Returns the selected tools and the index at which Rust attaches endpoint metadata.
/// The parsed array contains fixed optional-tool IDs, never paths or invocation data.
pub fn super_expose_tool_catalog(
    nodes: &[JsonNode<'_>],
    raw: &str,
    exec_only: bool,
    max_event_page: usize,
) -> Result<Vec<u8>, MojoError> {
    transform_json(
        nodes,
        raw,
        signed(max_event_page)?,
        exec_only,
        prodex_mojo_super_expose_tool_catalog_v1,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{JsonKind, StringView, ffi_nodes};

    #[test]
    fn compiled_tool_catalog_abi_checks_version_mode_limit_and_capacity() {
        let raw = "[]";
        let nodes = [JsonNode {
            kind: JsonKind::Array,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: raw.len(),
        }];
        let full = super_expose_tool_catalog(&nodes, raw, false, 17).unwrap();
        assert!(
            std::str::from_utf8(&full)
                .unwrap()
                .contains("\"maximum\":17")
        );
        assert!(full.ends_with(b"\"exec_index\":6}"));
        let exec = super_expose_tool_catalog(&nodes, raw, true, 17).unwrap();
        assert!(exec.ends_with(b"\"exec_index\":0}"));
        assert!(
            !std::str::from_utf8(&exec)
                .unwrap()
                .contains("prodex_super_start")
        );
        let input = ffi_nodes(&nodes, raw).unwrap();
        let mut scratch = [StringView::default(); 1];
        let mut output = [83_u8; 16];
        let mut metadata = [71_i64; 2];
        for (abi, page, mode, measuring, capacity, expected) in [
            (2, 64, 0, 0, 16, 4),
            (1, 0, 0, 0, 16, 1),
            (1, 64, 2, 0, 16, 1),
            (1, 64, 0, 2, 16, 1),
            (1, 64, 0, 0, 0, 3),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_super_expose_tool_catalog_v1(
                        abi,
                        page,
                        mode,
                        input.as_ptr() as u64,
                        input.len() as i64,
                        raw.as_ptr() as u64,
                        raw.len() as i64,
                        scratch.as_mut_ptr() as u64,
                        1,
                        measuring,
                        output.as_mut_ptr() as u64,
                        capacity,
                        metadata.as_mut_ptr() as u64,
                    )
                },
                expected
            );
            assert_eq!(metadata, [71; 2]);
            assert_eq!(output, [83; 16]);
        }
    }
}
