use super::*;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn super_choice_navigation_policy_is_mojo_backed() {
    assert_eq!(visible_choice_range(5, 10, 3), 4..7);
    assert_eq!(visible_choice_range(0, 10, 0), 0..1);

    let mut selected = 0;
    assert_eq!(
        update_super_choice_selection(key(KeyCode::Up), &mut selected, 5, false).unwrap(),
        None
    );
    assert_eq!(selected, 4);
    assert_eq!(
        update_super_choice_selection(key(KeyCode::Down), &mut selected, 5, false).unwrap(),
        None
    );
    assert_eq!(selected, 0);
    assert_eq!(
        update_super_choice_selection(key(KeyCode::PageDown), &mut selected, 5, false).unwrap(),
        None
    );
    assert_eq!(selected, 4);
    assert_eq!(
        update_super_choice_selection(key(KeyCode::Enter), &mut selected, 5, false).unwrap(),
        Some(4)
    );

    let mut vim_selected = 2;
    assert_eq!(
        update_super_choice_selection(key(KeyCode::Char('k')), &mut vim_selected, 5, false)
            .unwrap(),
        None
    );
    assert_eq!(vim_selected, 1);
    assert_eq!(
        update_super_choice_selection(key(KeyCode::Char('j')), &mut vim_selected, 5, false)
            .unwrap(),
        None
    );
    assert_eq!(vim_selected, 2);

    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(update_super_choice_selection(ctrl_c, &mut vim_selected, 5, false).is_err());

    let mut escaped = 1;
    assert_eq!(
        update_super_choice_selection(key(KeyCode::Esc), &mut escaped, 5, true).unwrap(),
        Some(4)
    );
    assert!(update_super_choice_selection(key(KeyCode::Esc), &mut escaped, 5, false).is_err());
}

#[test]
fn text_edit_caller_preserves_unicode_optional_escape_and_control_cancel() {
    assert_eq!(
        super_prompt_text_edit_plan(key(KeyCode::Char('é')), 255, false).unwrap(),
        (SuperPromptTextAction::Append, Some('é'), true)
    );
    assert_eq!(
        super_prompt_text_edit_plan(key(KeyCode::Char('x')), 256, false).unwrap(),
        (SuperPromptTextAction::Ignore, None, false)
    );
    assert_eq!(
        super_prompt_text_edit_plan(key(KeyCode::Backspace), 0, false).unwrap(),
        (SuperPromptTextAction::Backspace, None, true)
    );
    assert_eq!(
        super_prompt_text_edit_plan(key(KeyCode::Esc), 0, true)
            .unwrap()
            .0,
        SuperPromptTextAction::ReturnNone
    );
    assert_eq!(
        super_prompt_text_edit_plan(key(KeyCode::Esc), 0, false)
            .unwrap()
            .0,
        SuperPromptTextAction::Cancel
    );
    let ctrl_z = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL);
    assert_eq!(
        super_prompt_text_edit_plan(ctrl_z, 0, true).unwrap().0,
        SuperPromptTextAction::Cancel
    );
}

#[test]
fn concurrency_picker_uses_mojo_menu_and_selection_plan() {
    assert_eq!(
        super_sub_agent_concurrency_choices(),
        ["default (4)", "4", "8", "16", "32", "custom..."]
    );

    let default = prompt_super_sub_agent_max_concurrency_with(
        |choices| {
            assert_eq!(choices[0], "default (4)");
            Ok(0)
        },
        || panic!("default selection must skip custom input"),
    )
    .unwrap();
    assert_eq!(default, SubAgentMaxConcurrency::default());

    let preset = prompt_super_sub_agent_max_concurrency_with(
        |_| Ok(2),
        || panic!("preset selection must skip custom input"),
    )
    .unwrap();
    assert_eq!(preset.get(), 8);
    assert_eq!(preset.source(), SubAgentConcurrencySource::Preset);

    let custom =
        prompt_super_sub_agent_max_concurrency_with(|_| Ok(5), || Ok(Some("23".parse().unwrap())))
            .unwrap();
    assert_eq!(custom.get(), 23);
}

#[test]
fn provider_model_and_effort_prompt_choices_use_mojo_plans() {
    use prodex_provider_core::ProviderModelChoice;

    let choices = [
        ProviderModelChoice::ProviderDefault,
        ProviderModelChoice::Model("gpt-5.6-sol".to_string()),
        ProviderModelChoice::Custom,
    ];
    assert_eq!(
        super_prompt_model_choice_index(&choices, Some("GPT-5.6-SOL")),
        1
    );
    assert_eq!(super_prompt_model_choice_index(&choices, Some("other")), 0);
    assert_eq!(
        super_prompt_model_choice_label(&choices[0], "ignored"),
        "provider default"
    );
    assert_eq!(
        super_prompt_model_choice_label(&choices[2], "ignored"),
        "custom model..."
    );
    assert_eq!(
        super_prompt_provider_choice_label(prodex_provider_core::ProviderId::OpenAi, true),
        "OpenAI (session affinity)"
    );

    let efforts = [None, Some("low"), Some("high")];
    assert_eq!(
        super_prompt_effort_choice_index(&efforts, Some("high"), false),
        2
    );
    assert_eq!(
        super_prompt_effort_choice_index(&efforts, Some("HIGH"), false),
        0
    );
    assert_eq!(
        super_prompt_effort_choice_index(&efforts, Some("HIGH"), true),
        2
    );
}
