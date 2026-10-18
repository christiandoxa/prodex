//! Quota view assembly over host-acquired provider renderer facts.
use super::{JsonNode, transform_json_with_scratch};
use crate::MojoError;

#[derive(Clone, Copy, Debug)]
#[repr(i64)]
pub enum QuotaWatchPresentation {
    Row,
    ProfileFields,
    CompactLabel,
}

unsafe extern "C" {
    fn prodex_quota_watch_presentation_v1(
        abi: i64,
        operation: i64,
        detail: i64,
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
}

pub fn quota_watch_presentation(
    nodes: &[JsonNode<'_>],
    raw: &str,
    operation: QuotaWatchPresentation,
    detail: bool,
) -> Result<Vec<u8>, MojoError> {
    let bytes_per_node = nodes
        .len()
        .checked_mul(16)
        .filter(|count| *count > 0)
        .ok_or(MojoError::InvalidInput)?;
    let scratch_bytes = raw.len().checked_add(128).ok_or(MojoError::Capacity)?;
    transform_json_with_scratch(
        nodes,
        raw,
        operation as i64,
        detail,
        prodex_quota_watch_presentation_v1,
        scratch_bytes.div_ceil(bytes_per_node).max(1),
    )?
    .ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{JsonKind, ffi_nodes};

    fn root(text: &str) -> JsonNode<'_> {
        JsonNode {
            kind: JsonKind::String,
            first_child: None,
            next_sibling: None,
            parent: None,
            key: "",
            text,
            raw_start: 0,
            raw_length: 2,
        }
    }

    #[test]
    fn mojo_quota_compact_labels_preserve_unicode_leading_space_and_exact_prefixes() {
        for (text, expected) in [
            ("\u{2003}Account: name 🧭", b"true".as_slice()),
            ("Weekly remaining pool: 90%", b"true"),
            ("Model provider: example", b"true"),
            ("Source: file", b"true"),
            ("Account : name", b"false"),
            ("account: name", b"false"),
            ("Account", b"false"),
            ("Unknown: value", b"false"),
        ] {
            assert_eq!(
                quota_watch_presentation(
                    &[root(text)],
                    "\"\"",
                    QuotaWatchPresentation::CompactLabel,
                    false
                )
                .unwrap(),
                expected,
                "{text}"
            );
        }
    }

    #[test]
    fn mojo_quota_presentation_abi_rejects_invalid_contracts_before_output_writes() {
        let mut nodes = ffi_nodes(&[root("Account: test")], "\"\"").unwrap();
        let raw = b"\"\"";
        let mut scratch = [0_u64; 32];
        let mut output = [61_u8; 16];
        let mut metadata = [71_i64; 2];
        let source = nodes.as_ptr() as usize as u64;
        let scratch_address = scratch.as_mut_ptr() as usize as u64;
        let out = output.as_mut_ptr() as usize as u64;
        let meta = metadata.as_mut_ptr() as usize as u64;
        let call = |abi,
                    operation,
                    detail,
                    source,
                    count,
                    scratch_count,
                    measuring,
                    out,
                    capacity,
                    meta| unsafe {
            prodex_quota_watch_presentation_v1(
                abi,
                operation,
                detail,
                source,
                count,
                raw.as_ptr() as usize as u64,
                2,
                scratch_address,
                scratch_count,
                measuring,
                out,
                capacity,
                meta,
            )
        };
        assert_eq!(call(2, 2, 0, source, 1, 16, 0, out, 16, meta), 4);
        assert_eq!(call(1, 3, 0, source, 1, 16, 0, out, 16, meta), 1);
        assert_eq!(call(1, 2, 2, source, 1, 16, 0, out, 16, meta), 1);
        assert_eq!(call(1, 2, 0, 0, 1, 16, 0, out, 16, meta), 1);
        assert_eq!(call(1, 2, 0, source, 0, 16, 0, out, 16, meta), 1);
        assert_eq!(call(1, 2, 0, source, 1, 0, 0, out, 16, meta), 1);
        assert_eq!(call(1, 2, 0, source, 1, 16, 2, out, 16, meta), 1);
        assert_eq!(call(1, 2, 0, source, 1, 16, 0, 0, 16, meta), 1);
        assert_eq!(call(1, 2, 0, source, 1, 16, 0, out, 16, 0), 1);
        assert_eq!(call(1, 2, 0, source, 1, 16, 0, out, 1, meta), 3);
        nodes[0].first_child = 0;
        assert_eq!(call(1, 2, 0, source, 1, 16, 0, out, 16, meta), 1);
        assert_eq!(output, [61; 16]);
        assert_eq!(metadata, [71; 2]);
    }
}
