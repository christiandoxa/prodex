use super::*;

fn preserved_catalog() -> Vec<Value> {
    serde_json::from_str(include_str!("tool_catalog_baseline.json")).unwrap()
}

#[test]
fn compiled_mcp_catalog_preserves_complete_schemas_order_descriptions_and_annotations() {
    let optional_tools = ExposeOptionalTools::empty_for_tests();
    let mut actual = tools(SuperExposeMode::Full, &optional_tools);
    let metadata = actual[6].as_object_mut().unwrap().remove("_meta");
    assert_eq!(
        metadata,
        Some(json!({"prodex/optionalTools":optional_tools.public_manifest()}))
    );
    assert_eq!(actual, preserved_catalog());
}

#[test]
fn compiled_mcp_exec_catalog_uses_playwright_aliases_and_keeps_endpoint_metadata_in_rust() {
    let optional_tools = ExposeOptionalTools::with_tool_for_tests(
        prodex_optional_tools::OptionalToolId::PlaywrightMcp,
        Some(std::path::PathBuf::from("/synthetic/playwright")),
        std::path::PathBuf::from("/synthetic/prodex"),
    );
    let mut actual = tools(SuperExposeMode::Exec, &optional_tools);
    assert_eq!(actual.len(), 1);
    let metadata = actual[0].as_object_mut().unwrap().remove("_meta");
    assert_eq!(
        metadata,
        Some(json!({"prodex/optionalTools":optional_tools.public_manifest()}))
    );
    let mut expected = preserved_catalog().remove(6);
    let base = expected["description"]
        .as_str()
        .unwrap()
        .strip_suffix(" No validated Prodex optional-tool aliases are active for this endpoint.")
        .unwrap();
    expected["description"] = json!(format!(
        "{base} Validated optional-tool aliases available at endpoint start: optional:playwright-mcp, optional:playwright. Prefer these aliases over guessing PATH locations."
    ));
    assert_eq!(actual, vec![expected]);
}
