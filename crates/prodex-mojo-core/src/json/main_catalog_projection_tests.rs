use super::main_catalog_projection_json;
use crate::{
    MojoError,
    json::{JsonKind, JsonNode},
};

#[test]
fn compiled_main_catalog_projection_preserves_complete_alias_and_default_observations() {
    let raw = "[{\"slug\":null,\"id\":\" demo \",\"aliases\":[\" a \",\"\",true]}]";
    let nodes = [
        (JsonKind::Array, None, Some(1), None, "", "", 0, 55),
        (JsonKind::Object, Some(0), Some(2), None, "", "", 1, 53),
        (JsonKind::Null, Some(1), None, Some(3), "slug", "", 9, 4),
        (
            JsonKind::String,
            Some(1),
            None,
            Some(4),
            "id",
            " demo ",
            19,
            8,
        ),
        (
            JsonKind::Array,
            Some(1),
            Some(5),
            None,
            "aliases",
            "",
            38,
            15,
        ),
        (JsonKind::String, Some(4), None, Some(6), "", " a ", 39, 5),
        (JsonKind::String, Some(4), None, Some(7), "", "", 45, 2),
        (JsonKind::True, Some(4), None, None, "", "", 48, 4),
    ]
    .into_iter()
    .map(
        |(kind, parent, first_child, next_sibling, key, text, raw_start, raw_length)| JsonNode {
            kind,
            parent,
            first_child,
            next_sibling,
            key,
            text,
            raw_start,
            raw_length,
        },
    )
    .collect::<Vec<_>>();
    let output = main_catalog_projection_json(&nodes, raw).unwrap();
    assert_eq!(
        std::str::from_utf8(&output).unwrap(),
        "[{\"id\":\"demo\",\"label\":\"demo\",\"priority\":9223372036854775807,\"supported\":true,\"hidden\":false,\"listed\":true,\"efforts\":[],\"aliases\":[\"a\"],\"default_effort\":null}]"
    );
}

#[test]
fn compiled_main_catalog_projection_accepts_empty_arrays_and_rejects_invalid_roots_and_links() {
    let root = JsonNode {
        kind: JsonKind::Array,
        parent: None,
        first_child: None,
        next_sibling: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: 2,
    };
    assert_eq!(main_catalog_projection_json(&[root], "[]").unwrap(), b"[]");
    assert_eq!(
        main_catalog_projection_json(&[], "[]"),
        Err(MojoError::InvalidInput)
    );
    let mut invalid = root;
    invalid.kind = JsonKind::Object;
    assert_eq!(
        main_catalog_projection_json(&[invalid], "{}"),
        Err(MojoError::InvalidInput)
    );
    let mut invalid = root;
    invalid.first_child = Some(1);
    assert_eq!(
        main_catalog_projection_json(&[invalid], "[]"),
        Err(MojoError::InvalidInput)
    );
}
