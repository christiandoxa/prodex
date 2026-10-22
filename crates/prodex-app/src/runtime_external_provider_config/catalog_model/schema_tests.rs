use super::{ExternalCatalogProvider, external_catalog_model};
use serde_json::Value;

#[test]
fn external_runtime_schema_preserves_original_catalog_objects() {
    let outputs = [
        (
            ExternalCatalogProvider::Copilot,
            "synthetic",
            "Display",
            "Description",
            1,
            200_000,
            190_000,
        ),
        (
            ExternalCatalogProvider::Kiro,
            "auto",
            "Kiro",
            "Catalog schema",
            2,
            321,
            123,
        ),
        (
            ExternalCatalogProvider::Anthropic,
            "claude-opus-4-8",
            "Claude",
            "Catalog schema",
            3,
            400_000,
            100_000,
        ),
        (
            ExternalCatalogProvider::Copilot,
            " whitespace\"\\雪 ",
            "Display\n雪",
            "Description\t\\\"",
            usize::MAX,
            u64::MAX,
            0,
        ),
    ]
    .map(
        |(provider, slug, name, description, priority, context, compact)| {
            external_catalog_model(
                provider,
                slug,
                name,
                description,
                priority,
                context,
                compact,
            )
            .unwrap()
        },
    );
    let expected: Vec<Value> = serde_json::from_str(include_str!("schema_baseline.json")).unwrap();
    assert_eq!(outputs.as_slice(), expected.as_slice());
}
