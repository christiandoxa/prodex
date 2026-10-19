use super::*;

#[test]
fn compiled_optional_presentation_preserves_every_invocation_kind_and_version_shape() {
    let expected: Value =
        serde_json::from_str(include_str!("optional_tools_presentation_baseline.json")).unwrap();
    let versions = [None, Some(""), Some("1.2.3"), Some("v\"quoted\"\nβ\0")];
    let mut context = ExposeOptionalTools::empty_for_tests();
    for (index, tool) in context.tools.iter_mut().enumerate() {
        tool.usable = true;
        tool.version = versions[index % 4].map(str::to_string);
    }
    assert_eq!(
        json!({"instructions":context.instructions(),"manifest":context.public_manifest()}),
        expected
    );
}

#[test]
fn compiled_optional_unavailable_instructions_hide_versions_while_manifest_keeps_observations() {
    let mut context = ExposeOptionalTools::empty_for_tests();
    for tool in &mut context.tools {
        tool.version = Some("diagnostic-only".to_string());
    }
    assert_eq!(
        context.instructions(),
        concat!(
            "Optional tools are validated once when this expose endpoint starts. Use only entries marked available; unavailable or incompatible tools are not active.\n",
            "Use program optional:<name> for validated aliases. Normal program names and paths keep their existing behavior.\n",
            "- caveman: unavailable.\n- rtk: unavailable.\n- codebase-memory-mcp: unavailable.\n- playwright-mcp: unavailable.\n- ponytail: unavailable.\n- presidio: unavailable."
        )
    );
    let mut expected: Value =
        serde_json::from_str(include_str!("optional_tools_presentation_baseline.json")).unwrap();
    for tool in expected["manifest"].as_array_mut().unwrap() {
        tool["available"] = json!(false);
        tool["version"] = json!("diagnostic-only");
    }
    assert_eq!(context.public_manifest(), expected["manifest"]);
}

#[test]
fn compiled_optional_presentation_rejects_malformed_observations() {
    use prodex_mojo_core::json::{
        SuperExposeOptionalToolOperation, super_expose_optional_tool_presentation,
    };
    for facts in [
        json!([null]),
        json!([{"id":"unknown","kind":"Command","available":false,"version":null}]),
        json!([{"id":"rtk","kind":7,"available":false,"version":null}]),
        json!([{"id":"rtk","kind":"Command","available":"false","version":null}]),
        json!([{"id":"rtk","kind":"Command","available":false,"version":7}]),
        json!([{"id":"rtk","kind":"Command","available":false}]),
    ] {
        let tree = crate::json_boundary::app_json_tree(&facts).unwrap();
        assert_eq!(
            super_expose_optional_tool_presentation(
                &tree.nodes,
                std::str::from_utf8(&tree.raw).unwrap(),
                SuperExposeOptionalToolOperation::Instructions,
            ),
            Err(prodex_mojo_core::MojoError::InvalidInput)
        );
    }
}
