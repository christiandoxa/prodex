use super::*;

#[test]
fn login_menu_entries_explain_runtime_only_api_key_providers() {
    let entries = login_menu_entries();
    assert!(entries.len() > 5);

    let deepseek = entries
        .iter()
        .find(|entry| entry.title == "DeepSeek API key")
        .expect("deepseek guidance should be listed");
    assert_eq!(
        deepseek.action,
        LoginMenuAction::Guidance(LoginGuidanceKind::DeepSeekApiKey)
    );
    assert_eq!(deepseek.auth, "Runtime API key only");
    assert!(deepseek.usage.contains("no OAuth login"));

    let gemini_api_key = entries
        .iter()
        .find(|entry| entry.title == "Google Gemini API key")
        .expect("gemini API key guidance should be listed");
    assert_eq!(
        gemini_api_key.action,
        LoginMenuAction::Guidance(LoginGuidanceKind::GeminiApiKey)
    );
    assert!(gemini_api_key.command.contains("GEMINI_API_KEY"));

    assert!(
        entries
            .iter()
            .all(|entry| entry.title != "Google Gemini OAuth")
    );
}

#[test]
fn login_menu_entries_keep_all_auth_surfaces_described() {
    let entries = login_menu_entries();
    for entry in entries {
        assert!(!entry.title.is_empty());
        assert!(!entry.provider.is_empty());
        assert!(!entry.auth.is_empty());
        assert!(!entry.usage.is_empty());
        assert!(!entry.command.is_empty());
    }
}

#[test]
fn copilot_login_menu_selection_runs_import_instead_of_guidance() {
    assert_eq!(
        super::super::classify_login_menu_action(LoginMenuAction::Guidance(
            LoginGuidanceKind::CopilotImport,
        )),
        super::super::PromptLoginSelection::ImportCopilot
    );
}

#[test]
fn api_key_guidance_still_shows_guidance() {
    assert_eq!(
        super::super::classify_login_menu_action(LoginMenuAction::Guidance(
            LoginGuidanceKind::DeepSeekApiKey,
        )),
        super::super::PromptLoginSelection::Guidance(LoginGuidanceKind::DeepSeekApiKey)
    );
}

#[test]
fn login_menu_navigation_policy_is_mojo_backed() {
    let entries = login_menu_entries();
    let compact = login_menu_layout_for_rows(12, entries.len());
    assert!(compact.compact);
    assert_eq!(compact.visible_items, 7);
    let roomy = login_menu_layout_for_rows(24, entries.len());
    assert!(!roomy.compact);
    assert_eq!(roomy.visible_items, entries.len());

    assert_eq!(login_menu_window_offset(6, 0, 4, entries.len()), 3);
    assert_eq!(login_menu_window_offset(1, 3, 4, entries.len()), 1);

    let mut selected = 2;
    assert_eq!(
        apply_login_menu_key(LoginMenuKey::Down, &mut selected, 4, entries).unwrap(),
        None
    );
    assert_eq!(selected, 3);
    assert_eq!(
        apply_login_menu_key(LoginMenuKey::PageUp, &mut selected, 4, entries).unwrap(),
        None
    );
    assert_eq!(selected, 0);

    assert_eq!(
        apply_login_menu_key(LoginMenuKey::Digit(9), &mut selected, 4, entries).unwrap(),
        Some(entries[8].action)
    );
    assert!(apply_login_menu_key(LoginMenuKey::Cancel, &mut selected, 4, entries).is_err());
}
