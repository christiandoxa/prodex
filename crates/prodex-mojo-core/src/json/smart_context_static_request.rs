//! Complete static-prompt request projection and allocation-free classifiers.
use super::{JsonNode, status, transform_json_with_scratch};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_smart_context_static_request_v1(
        abi: i64,
        operation: i64,
        flag: i64,
        nodes: u64,
        count: i64,
        raw: u64,
        raw_length: i64,
        scratch: u64,
        scratch_count: i64,
        measuring: i64,
        output: u64,
        capacity: i64,
        metadata: u64,
    ) -> i64;
    fn prodex_smart_context_static_boundary_v1(
        abi: i64,
        operation: i64,
        address: u64,
        length: i64,
        result: u64,
    ) -> i64;
}

pub fn smart_context_static_request(
    nodes: &[JsonNode<'_>],
    raw: &str,
) -> Result<Vec<u8>, MojoError> {
    transform_json_with_scratch(
        nodes,
        raw,
        0,
        false,
        prodex_smart_context_static_request_v1,
        7,
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum StaticBoundary {
    FieldKey,
    Role,
}

fn static_boundary(text: &str, operation: StaticBoundary) -> Result<bool, MojoError> {
    let mut result = -1_i64;
    status(unsafe {
        prodex_smart_context_static_boundary_v1(
            1,
            operation as i64,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn smart_context_static_prompt_key(key: &str) -> Result<bool, MojoError> {
    static_boundary(key, StaticBoundary::FieldKey)
}

pub fn smart_context_static_prompt_role(role: &str) -> Result<bool, MojoError> {
    static_boundary(role, StaticBoundary::Role)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{JsonKind, ffi_nodes};

    fn empty() -> JsonNode<'static> {
        JsonNode {
            kind: JsonKind::Object,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 2,
        }
    }

    #[test]
    fn mojo_static_request_empty_projection_and_exact_boundary_contract() {
        assert_eq!(
            smart_context_static_request(&[empty()], "{}").unwrap(),
            br#"{"items":[],"legacy_hash":null}"#
        );
        for (value, field, role) in [
            ("instructions", true, false),
            ("system", true, true),
            ("developer", true, true),
            ("user", false, false),
            ("SYSTEM", false, false),
            (" system", false, false),
            ("developer\u{2003}", false, false),
            ("", false, false),
            ("東京🧭", false, false),
        ] {
            assert_eq!(smart_context_static_prompt_key(value), Ok(field), "{value}");
            assert_eq!(smart_context_static_prompt_role(value), Ok(role), "{value}");
        }
    }

    #[test]
    fn mojo_static_request_abi_rejects_invalid_views_and_short_buffers_before_writes() {
        let nodes = ffi_nodes(&[empty()], "{}").unwrap();
        let raw = b"{}";
        let mut scratch = [0_u64; 14];
        let mut output = [61_u8; 64];
        let mut metadata = [71_i64; 2];
        let input = nodes.as_ptr() as usize as u64;
        let buffer = output.as_mut_ptr() as usize as u64;
        let meta = metadata.as_mut_ptr() as usize as u64;
        let mut call =
            |abi, op, flag, input, count, scratch_count, measuring, out, capacity, meta| unsafe {
                prodex_smart_context_static_request_v1(
                    abi,
                    op,
                    flag,
                    input,
                    count,
                    raw.as_ptr() as usize as u64,
                    2,
                    scratch.as_mut_ptr() as usize as u64,
                    scratch_count,
                    measuring,
                    out,
                    capacity,
                    meta,
                )
            };
        assert_eq!(call(2, 0, 0, input, 1, 7, 0, buffer, 64, meta), 4);
        assert_eq!(call(1, 1, 0, input, 1, 7, 0, buffer, 64, meta), 1);
        assert_eq!(call(1, 0, 1, input, 1, 7, 0, buffer, 64, meta), 1);
        assert_eq!(call(1, 0, 0, 0, 1, 7, 0, buffer, 64, meta), 1);
        assert_eq!(call(1, 0, 0, input, 0, 7, 0, buffer, 64, meta), 1);
        assert_eq!(call(1, 0, 0, input, 1, 6, 0, buffer, 64, meta), 1);
        assert_eq!(call(1, 0, 0, input, 1, 7, 2, buffer, 64, meta), 1);
        assert_eq!(call(1, 0, 0, input, 1, 7, 0, 0, 64, meta), 1);
        assert_eq!(call(1, 0, 0, input, 1, 7, 0, buffer, 64, 0), 1);
        assert_eq!(call(1, 0, 0, input, 1, 7, 0, buffer, 1, meta), 3);
        assert_eq!(output, [61; 64]);
        assert_eq!(metadata, [71; 2]);
        let mut bad = empty();
        bad.first_child = Some(0);
        assert_eq!(
            smart_context_static_request(&[bad], "{}"),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn mojo_static_boundary_abi_rejects_invalid_utf8_and_flags_without_writes() {
        let bytes = [0xff_u8];
        let input = bytes.as_ptr() as usize as u64;
        let mut output = 71_i64;
        let dest = (&mut output as *mut i64) as usize as u64;
        let call = |abi, op, address, length, out| unsafe {
            prodex_smart_context_static_boundary_v1(abi, op, address, length, out)
        };
        assert_eq!(call(2, 0, input, 1, dest), 4);
        assert_eq!(call(1, 2, input, 1, dest), 1);
        assert_eq!(call(1, 0, input, -1, dest), 1);
        assert_eq!(call(1, 0, 0, 1, dest), 1);
        assert_eq!(call(1, 0, input, 1, 0), 1);
        assert_eq!(call(1, 0, input, 1, dest), 1);
        assert_eq!(output, 71);
    }
}
