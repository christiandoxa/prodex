use super::{LogTokenPresentation, log_token_presentation_json, prodex_log_token_presentation_v1};
use crate::{
    MojoError,
    json::{JsonKind, JsonNode, ffi_nodes},
};

const RAW: &str = r##"[{"timestamp":"雪\n\t\u0000\"\\","request":18446744073709551615,"profile":"profile-a","transport":"sse","source":"responses","input_tokens":18446744073709551615,"cached_input_tokens":0,"output_tokens":15,"reasoning_tokens":2,"generation_ms":1000,"output_tokens_per_second":15.0},"15.0 t/s","— t/s"]"##;

fn input() -> Vec<JsonNode<'static>> {
    use JsonKind::{Array, Number, Object, String};
    let mut nodes = vec![
        JsonNode {
            kind: Array,
            parent: None,
            first_child: Some(1),
            next_sibling: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: RAW.len(),
        },
        JsonNode {
            kind: Object,
            parent: Some(0),
            first_child: Some(2),
            next_sibling: Some(13),
            key: "",
            text: "",
            raw_start: 1,
            raw_length: 278,
        },
    ];
    for (index, (kind, key, text, start, length)) in [
        (String, "timestamp", "雪\n\t\0\"\\", 14, 19),
        (Number, "request", "", 44, 20),
        (String, "profile", "profile-a", 75, 11),
        (String, "transport", "sse", 99, 5),
        (String, "source", "responses", 114, 11),
        (Number, "input_tokens", "", 141, 20),
        (Number, "cached_input_tokens", "", 184, 1),
        (Number, "output_tokens", "", 202, 2),
        (Number, "reasoning_tokens", "", 224, 1),
        (Number, "generation_ms", "", 242, 4),
        (Number, "output_tokens_per_second", "", 274, 4),
    ]
    .into_iter()
    .enumerate()
    {
        nodes.push(JsonNode {
            kind,
            parent: Some(1),
            first_child: None,
            next_sibling: (index < 10).then_some(index + 3),
            key,
            text,
            raw_start: start,
            raw_length: length,
        });
    }
    nodes.push(JsonNode {
        kind: String,
        parent: Some(0),
        first_child: None,
        next_sibling: Some(14),
        key: "",
        text: "15.0 t/s",
        raw_start: 280,
        raw_length: 10,
    });
    nodes.push(JsonNode {
        kind: String,
        parent: Some(0),
        first_child: None,
        next_sibling: None,
        key: "",
        text: "— t/s",
        raw_start: 291,
        raw_length: 9,
    });
    nodes
}

#[test]
fn token_log_bridge_preserves_complete_projections_unicode_and_u64_endpoints() {
    let nodes = input();
    for (operation, expected) in [
        (
            LogTokenPresentation::Envelope,
            r##"{"event":"token_usage","fields":{"request":"18446744073709551615","transport":"sse","profile":"profile-a","source":"responses","input_tokens":18446744073709551615,"cached_input_tokens":0,"output_tokens":15,"reasoning_tokens":2,"generation_ms":1000,"output_tokens_per_second":15.0},"timestamp":"雪\n\t\u0000\"\\","request":18446744073709551615,"profile":"profile-a","transport":"sse","source":"responses","input_tokens":18446744073709551615,"cached_input_tokens":0,"output_tokens":15,"reasoning_tokens":2,"generation_ms":1000,"output_tokens_per_second":15.0}"##,
        ),
        (
            LogTokenPresentation::Metadata,
            r##"[["profile","profile-a"],["request","18446744073709551615"],["transport","sse"],["source","responses"],["input","18446744073709551615"],["cache","0"],["output","15"],["reasoning","2"],["generation","1000ms"],["avg_output","15.0 t/s"]]"##,
        ),
        (
            LogTokenPresentation::Tui,
            r##"[[["Muted","雪\n\t\u0000\"\\"],["Raw"," "],["Token","TOKENS"]],[["Muted","profile "],["Primary","profile-a"],["Muted"," request "],["Primary","18446744073709551615"],["Muted"," transport "],["Primary","sse"],["Muted"," source "],["Primary","responses"]],[["Muted","input "],["Metric","18446744073709551615"],["Muted"," cache "],["Accent","0"],["Muted"," output "],["Metric","15"],["Muted"," reasoning "],["Tool","2"],["Muted"," generation "],["Metric","1000ms"]],[["Muted","average "],["Metric","15.0 t/s"]]]"##,
        ),
        (
            LogTokenPresentation::Search,
            r##""雪\n\t\u0000\"\\ 18446744073709551615 sse profile-a responses 18446744073709551615 0 15 2""##,
        ),
    ] {
        assert_eq!(
            std::str::from_utf8(&log_token_presentation_json(&nodes, RAW, operation).unwrap())
                .unwrap(),
            expected
        );
    }
}

#[test]
fn token_log_bridge_rejects_wrong_fields_types_and_tree_links() {
    let mut nodes = input();
    nodes[2].key = "unknown";
    assert_eq!(
        log_token_presentation_json(&nodes, RAW, LogTokenPresentation::Tui),
        Err(MojoError::InvalidInput)
    );
    nodes = input();
    nodes[3].kind = JsonKind::True;
    assert_eq!(
        log_token_presentation_json(&nodes, RAW, LogTokenPresentation::Metadata),
        Err(MojoError::InvalidInput)
    );
    nodes = input();
    nodes[1].first_child = Some(nodes.len());
    assert_eq!(
        log_token_presentation_json(&nodes, RAW, LogTokenPresentation::Search),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn token_log_abi_preserves_short_or_invalid_output_and_metadata() {
    let nodes = ffi_nodes(&input(), RAW).unwrap();
    let mut scratch = [0_u64; 30];
    let mut output = [93_u8; 4096];
    let mut metadata = [77_i64, 88];
    let source = RAW.as_ptr() as u64;
    let target = output.as_mut_ptr() as u64;
    let input = nodes.as_ptr() as u64;
    let storage = scratch.as_mut_ptr() as u64;
    let meta = metadata.as_mut_ptr() as u64;
    let call = |version, operation, flag, capacity| unsafe {
        prodex_log_token_presentation_v1(
            version,
            operation,
            flag,
            input,
            15,
            source,
            RAW.len() as i64,
            storage,
            15,
            0,
            target,
            capacity,
            meta,
        )
    };
    assert_eq!(call(2, 0, 0, 4096), 4);
    assert_eq!(call(1, 4, 0, 4096), 1);
    assert_eq!(call(1, 0, 1, 4096), 1);
    assert_eq!(call(1, 0, 0, 1), 3);
    assert_eq!(output, [93_u8; 4096]);
    assert_eq!(metadata, [77_i64, 88]);
}
