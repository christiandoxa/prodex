use super::{JsonKind, JsonNode, provider_error_rejects_member};

#[test]
fn provider_error_member_kernel_matches_expected_object_fields() {
    let nodes = [
        JsonNode {
            kind: JsonKind::Object,
            first_child: Some(1),
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 2,
        },
        JsonNode {
            kind: JsonKind::String,
            first_child: None,
            next_sibling: Some(2),
            parent: Some(0),
            key: "code",
            text: "UNKNOWN_PARAMETER",
            raw_start: 0,
            raw_length: 0,
        },
        JsonNode {
            kind: JsonKind::String,
            first_child: None,
            next_sibling: None,
            parent: Some(0),
            key: "param",
            text: "WEB_search-options",
            raw_start: 0,
            raw_length: 0,
        },
    ];

    assert_eq!(
        provider_error_rejects_member(&nodes, "{}", "webSearchOptions"),
        Ok(true)
    );
}
