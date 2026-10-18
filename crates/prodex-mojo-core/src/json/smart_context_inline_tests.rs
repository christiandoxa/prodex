use super::*;
use crate::json::JsonKind;

const HASH: &str = "sc2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn request<'a>(left: &'a str, right: &'a str) -> (Vec<JsonNode<'a>>, String) {
    let mut raw = String::from("{\"input\":[");
    let array_start = raw.len() - 1;
    let mut spans = Vec::new();
    for text in [left, right] {
        if !spans.is_empty() {
            raw.push(',');
        }
        let object_start = raw.len();
        raw.push_str("{\"output\":");
        let text_start = raw.len();
        raw.push('"');
        raw.push_str(text);
        raw.push('"');
        let text_end = raw.len();
        raw.push('}');
        spans.push((object_start, raw.len(), text_start, text_end));
    }
    raw.push(']');
    let array_end = raw.len();
    raw.push('}');
    let node = |kind, parent, key, text, start, end| JsonNode {
        kind,
        first_child: None,
        next_sibling: None,
        parent,
        key,
        text,
        raw_start: start,
        raw_length: end - start,
    };
    let mut nodes = vec![
        node(JsonKind::Object, None, "", "", 0, raw.len()),
        node(
            JsonKind::Array,
            Some(0),
            "input",
            "",
            array_start,
            array_end,
        ),
    ];
    for (text, (object_start, object_end, text_start, text_end)) in
        [left, right].into_iter().zip(spans)
    {
        let parent = nodes.len();
        nodes.push(node(
            JsonKind::Object,
            Some(1),
            "",
            "",
            object_start,
            object_end,
        ));
        nodes.push(node(
            JsonKind::String,
            Some(parent),
            "output",
            text,
            text_start,
            text_end,
        ));
        nodes[parent].first_child = Some(parent + 1);
    }
    nodes[0].first_child = Some(1);
    nodes[1].first_child = Some(2);
    nodes[2].next_sibling = Some(4);
    (nodes, raw)
}

fn document<'a>(
    nodes: &'a [JsonNode<'a>],
    raw: &'a str,
    hashes: &'a [&'a str],
) -> SmartContextInlineDocument<'a> {
    SmartContextInlineDocument { nodes, raw, hashes }
}

#[test]
fn mojo_inline_reference_kernel_probes_rewrites_and_restores_unicode() {
    let text = "tool 🧭 data ".repeat(128);
    let (nodes, raw) = request(&text, &text);
    let hashes = vec![HASH; nodes.len()];
    let probe = smart_context_inline_plan(
        document(&nodes, &raw, &[]),
        None,
        SmartContextInlineOperation::Probe,
    )
    .unwrap();
    assert_eq!(
        probe,
        SmartContextInlinePlan {
            body: None,
            replacements: 1,
            candidate_limit_exceeded: false,
            valid: true
        }
    );
    let rewritten = smart_context_inline_plan(
        document(&nodes, &raw, &hashes),
        None,
        SmartContextInlineOperation::Rewrite,
    )
    .unwrap();
    let marker = format!(
        "[prodex-context-ref v=1 source=original-input[0] digest={HASH} bytes={}]",
        text.len()
    );
    let (candidate_nodes, candidate_raw) = request(&text, &marker);
    assert_eq!(rewritten.body.as_deref(), Some(candidate_raw.as_bytes()));
    assert_eq!(rewritten.replacements, 1);
    let expanded = smart_context_inline_plan(
        document(&nodes, &raw, &hashes),
        Some(document(&candidate_nodes, &candidate_raw, &[])),
        SmartContextInlineOperation::Expand,
    )
    .unwrap();
    assert_eq!(expanded.body.as_deref(), Some(raw.as_bytes()));
    assert_eq!(expanded.replacements, 1);
}

#[test]
fn mojo_inline_reference_kernel_retains_legacy_hashes_and_usize_parse_grammar() {
    let (nodes, raw) = request("hello", "other");
    let hashes = vec![HASH; nodes.len()];
    let marker =
        "[prodex-context-ref v=1 source=original-input[+00] digest=sc:a430d84680aabd0b bytes=+005]";
    let (candidate_nodes, candidate_raw) = request("hello", marker);
    let expanded = smart_context_inline_plan(
        document(&nodes, &raw, &hashes),
        Some(document(&candidate_nodes, &candidate_raw, &[])),
        SmartContextInlineOperation::Expand,
    )
    .unwrap();
    assert_eq!(
        expanded.body.as_deref(),
        Some(request("hello", "hello").1.as_bytes())
    );
}

#[test]
fn mojo_inline_reference_kernel_rejects_tampering_overflow_and_unknown_versions() {
    let (nodes, raw) = request("hello", "other");
    let hashes = vec![HASH; nodes.len()];
    for marker in [
        format!("[prodex-context-ref v=2 source=original-input[0] digest={HASH} bytes=5]"),
        format!("[prodex-context-ref v=1 source=original-input[2] digest={HASH} bytes=5]"),
        format!(
            "[prodex-context-ref v=1 source=original-input[18446744073709551616] digest={HASH} bytes=5]"
        ),
        format!(
            "[prodex-context-ref v=1 source=original-input[0] digest={HASH} bytes=18446744073709551616]"
        ),
        format!("[prodex-context-ref v=1 source=original-input[-1] digest={HASH} bytes=5]"),
        format!("[prodex-context-ref v=1 source=original-input[0] digest={HASH} bytes=6]"),
        "[prodex-context-ref v=1 source=original-input[0] digest=sc2:bad bytes=5]".to_string(),
        "[prodex-context-ref v=1 source=original-input[0] digest=sc:a430d84680aabd0c bytes=5]"
            .to_string(),
        "[prodex-context-ref v=1 source=original-input[0] digest=psc:sc:a430d84680aabd0b bytes=5]"
            .to_string(),
    ] {
        let (candidate_nodes, candidate_raw) = request("hello", &marker);
        assert_eq!(
            smart_context_inline_plan(
                document(&nodes, &raw, &hashes),
                Some(document(&candidate_nodes, &candidate_raw, &[])),
                SmartContextInlineOperation::Expand,
            )
            .unwrap(),
            SmartContextInlinePlan {
                body: None,
                replacements: 0,
                candidate_limit_exceeded: false,
                valid: false
            },
            "{marker}"
        );
    }
}

#[test]
fn mojo_inline_reference_abi_rejects_invalid_input_and_capacity_before_writes() {
    let text = "same tool data ".repeat(128);
    let (nodes, raw) = request(&text, &text);
    let mut ffi = ffi_nodes(&nodes, &raw).unwrap();
    let hashes = vec![StringView::from(HASH); nodes.len()];
    let mut scratch = vec![0_i64; nodes.len() * 12 + 1];
    let mut input = InlineInputFfi {
        source: InlineTreeFfi::new(&ffi, &raw, &hashes).unwrap(),
        candidate: InlineTreeFfi::default(),
        operation: 1,
        scratch: scratch.as_mut_ptr() as usize as u64,
        scratch_words: scratch.len() as i64,
    };
    let mut output = [61_u8; 8];
    let mut metadata = [71_i64; 3];
    let address = (&input as *const InlineInputFfi) as usize as u64;
    let dest = output.as_mut_ptr() as usize as u64;
    let meta = metadata.as_mut_ptr() as usize as u64;
    let call = |abi, input, measuring, out, capacity, meta, count| unsafe {
        prodex_smart_context_inline_v1(abi, input, measuring, out, capacity, meta, count)
    };
    assert_eq!(call(2, address, 0, dest, 8, meta, 3), 4);
    assert_eq!(call(1, 0, 0, dest, 8, meta, 3), 1);
    assert_eq!(call(1, address, 2, dest, 8, meta, 3), 1);
    assert_eq!(call(1, address, 0, 0, 8, meta, 3), 1);
    assert_eq!(call(1, address, 0, dest, 8, 0, 3), 1);
    assert_eq!(call(1, address, 0, dest, 8, meta, 2), 1);
    assert_eq!(call(1, address, 0, dest, 8, meta, 3), 3);
    let invoke = |input: &InlineInputFfi| {
        call(
            1,
            (input as *const InlineInputFfi) as usize as u64,
            0,
            dest,
            8,
            meta,
            3,
        )
    };
    input.scratch_words = 0;
    assert_eq!(invoke(&input), 1);
    input.scratch_words = scratch.len() as i64;
    input.operation = 7;
    assert_eq!(invoke(&input), 1);
    input.operation = 1;
    input.source.hash_count = 0;
    assert_eq!(invoke(&input), 1);
    input.source.hash_count = hashes.len() as i64;
    input.source.nodes = 0;
    assert_eq!(invoke(&input), 1);
    input.source.nodes = ffi.as_ptr() as usize as u64;
    ffi[0].first_child = 0;
    assert_eq!(invoke(&input), 1);
    assert_eq!(output, [61; 8]);
    assert_eq!(metadata, [71; 3]);
}

#[test]
fn mojo_inline_reference_kernel_is_stateless_across_parallel_calls() {
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                let text = "parallel tool data ".repeat(128);
                let (nodes, raw) = request(&text, &text);
                for _ in 0..8 {
                    let result = smart_context_inline_plan(
                        document(&nodes, &raw, &[]),
                        None,
                        SmartContextInlineOperation::Probe,
                    )
                    .unwrap();
                    assert_eq!(result.replacements, 1);
                    assert!(!result.candidate_limit_exceeded);
                }
            });
        }
    });
}
