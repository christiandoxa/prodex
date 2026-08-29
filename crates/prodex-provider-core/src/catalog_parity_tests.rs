use super::*;

#[test]
fn mojo_catalog_identity_choices_and_merge_match_rust_oracles() {
    for &provider in crate::PROVIDER_IMPLEMENTATION_ORDER {
        let entries = provider_catalog_entries_for(provider);
        let configured = [
            entries
                .first()
                .and_then(|entry| entry.aliases.first())
                .cloned()
                .unwrap_or_else(|| entries[0].id.clone()),
            format!("catalog-dynamic-{}", provider.label()),
            format!("CATALOG-DYNAMIC-{}", provider.label()),
        ];
        assert_eq!(
            resolve_provider_model_choices(provider, &configured, Some("current-model")),
            resolve_provider_model_choices_rust(provider, &configured, Some("current-model")),
        );

        let queries = entries
            .iter()
            .flat_map(|entry| {
                std::iter::once(entry.id.clone())
                    .chain(std::iter::once(entry.id.to_ascii_uppercase()))
                    .chain(entry.aliases.iter().cloned())
                    .chain(entry.aliases.iter().map(|alias| alias.to_ascii_uppercase()))
            })
            .chain(std::iter::once("unknown-model".to_string()));
        for query in queries {
            assert_eq!(
                provider_catalog_entry(provider, &query).map(|entry| entry.id.as_str()),
                provider_catalog_entry_rust(provider, &query).map(|entry| entry.id.as_str()),
            );
            let rust_model = crate::provider_model_catalog(provider)
                .iter()
                .find(|model| model.matches_id_or_alias(&query));
            assert_eq!(
                crate::provider_model_spec(provider, &query).map(|model| model.id),
                rust_model.map(|model| model.id),
            );
        }

        let additional = [
            entries[0].id.clone(),
            format!("catalog-dynamic-{}", provider.label()),
            format!("CATALOG-DYNAMIC-{}", provider.label()),
            format!(" catalog-spaced-{} ", provider.label()),
            "".to_string(),
        ];
        let additional_refs = additional.iter().map(String::as_str).collect::<Vec<_>>();
        assert_eq!(
            merge_catalog_ids_with_mojo(provider, &additional_refs),
            merge_catalog_ids_rust(provider, &additional_refs),
        );
    }
}
