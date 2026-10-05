#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::json::{
    JsonKind, JsonNode, KiroModelCatalogModel, KiroModelCatalogPlan, kiro_model_catalog_plan,
};

fn node(
    kind: JsonKind,
    key: &'static str,
    text: &'static str,
    parent: Option<usize>,
    first_child: Option<usize>,
    next_sibling: Option<usize>,
) -> JsonNode<'static> {
    JsonNode {
        kind,
        first_child,
        next_sibling,
        parent,
        key,
        text,
        raw_start: 0,
        raw_length: 0,
    }
}

fn catalog_tree() -> Vec<JsonNode<'static>> {
    vec![
        node(JsonKind::Object, "", "", None, Some(1), None),
        node(JsonKind::Array, "models", "", Some(0), Some(2), None),
        node(JsonKind::Object, "", "", Some(1), Some(3), Some(8)),
        node(
            JsonKind::String,
            "id",
            " \u{2003}primary\t",
            Some(2),
            None,
            Some(4),
        ),
        node(
            JsonKind::String,
            "model_id",
            "secondary",
            Some(2),
            None,
            Some(5),
        ),
        node(JsonKind::String, "name", " \t", Some(2), None, Some(6)),
        node(
            JsonKind::String,
            "modelName",
            "Display",
            Some(2),
            None,
            Some(7),
        ),
        node(
            JsonKind::String,
            "description",
            "detail",
            Some(2),
            None,
            None,
        ),
        node(JsonKind::Object, "", "", Some(1), Some(9), None),
        node(JsonKind::String, "modelId", "PRIMARY", Some(8), None, None),
    ]
}

#[test]
fn kiro_catalog_plan_uses_alias_precedence_unicode_trim_and_stable_source_order() {
    let plan = kiro_model_catalog_plan(&catalog_tree(), "{}", 1_024)
        .expect("required Mojo catalog planner succeeds");

    assert_eq!(
        plan,
        KiroModelCatalogPlan::Ready {
            input_count: 2,
            models: vec![
                KiroModelCatalogModel {
                    id: "primary".to_string(),
                    name: "Display".to_string(),
                    description: Some("detail".to_string()),
                    context_window_tokens: None,
                },
                KiroModelCatalogModel {
                    id: "PRIMARY".to_string(),
                    name: "PRIMARY".to_string(),
                    description: None,
                    context_window_tokens: None,
                },
            ],
        }
    );
}

#[test]
fn kiro_catalog_plan_returns_typed_missing_empty_and_limit_issues() {
    let missing = vec![node(JsonKind::Object, "", "", None, None, None)];
    assert_eq!(
        kiro_model_catalog_plan(&missing, "{}", 1_024).unwrap(),
        KiroModelCatalogPlan::MissingModelsArray
    );

    let unusable = vec![
        node(JsonKind::Array, "", "", None, Some(1), None),
        node(JsonKind::Object, "", "", Some(0), Some(2), None),
        node(JsonKind::String, "id", " \t", Some(1), None, None),
    ];
    assert_eq!(
        kiro_model_catalog_plan(&unusable, "{}", 1_024).unwrap(),
        KiroModelCatalogPlan::NoUsableModels
    );

    assert_eq!(
        kiro_model_catalog_plan(&catalog_tree(), "{}", 1).unwrap(),
        KiroModelCatalogPlan::TooManyModels { input_count: 2 }
    );
}
