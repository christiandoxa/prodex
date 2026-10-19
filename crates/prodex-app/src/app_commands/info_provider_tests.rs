use super::*;

fn profile_providers() -> Vec<prodex_state::ProfileProvider> {
    use prodex_state::ProfileProvider;
    vec![
        ProfileProvider::Openai,
        ProfileProvider::Gemini {
            email: "synthetic@example.com".to_string(),
            project_id: None,
        },
        ProfileProvider::Anthropic {
            account: None,
            auth_method: None,
        },
        ProfileProvider::Copilot {
            host: "example.com".to_string(),
            login: "synthetic".to_string(),
            api_url: "https://example.com".to_string(),
            access_type_sku: None,
            copilot_plan: None,
        },
        ProfileProvider::Kiro {
            auth_key: "fixture-key-reference".to_string(),
            auth_kind: None,
            profile_arn: None,
            profile_name: None,
            start_url: None,
            region: None,
        },
        ProfileProvider::Agy { account: None },
    ]
}

#[test]
fn info_provider_summaries_keep_empty_output() {
    let profiles = BTreeMap::new();
    assert_eq!(format_info_provider_summary(&profiles), "none");
    assert_eq!(format_info_provider_capabilities_summary(&profiles), "none");
}

#[test]
fn info_provider_summaries_group_all_provider_kinds_and_duplicate_labels() {
    let providers = profile_providers();
    let profiles = providers
        .iter()
        .chain(providers.iter())
        .enumerate()
        .map(|(index, provider)| {
            (
                format!("profile_{index:03}"),
                ProfileEntry {
                    codex_home: PathBuf::from("/synthetic/codex"),
                    managed: false,
                    email: None,
                    provider: provider.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        format_info_provider_summary(&profiles),
        "agy=2, anthropic=2, copilot=2, gemini=2, kiro=2, openai=2"
    );
    assert_eq!(
        format_info_provider_capabilities_summary(&profiles),
        "routes native=2, adapter=8, external-cli=2, unsupported=0; openai-format=10; quota copilot-monthly=2, external-status=6, gemini-buckets=2, openai-windows=2"
    );
}
