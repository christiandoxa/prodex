use std::ffi::OsString;

use prodex_cli::{SuperExternalProvider, super_external_provider_codex_args};

fn rendered_args(args: Vec<OsString>) -> Vec<String> {
    args.into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn super_external_provider_codex_args_use_copilot_default_model_prompt_budget() {
    let rendered = rendered_args(super_external_provider_codex_args(
        SuperExternalProvider::Copilot,
        "https://api.githubcopilot.com",
        Some("gpt-5.3-codex"),
        None,
        None,
    ));

    assert!(rendered.contains(&"model_context_window=272000".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=258400".to_string()));
}

#[test]
fn super_external_provider_codex_args_use_copilot_model_specific_prompt_budget() {
    let rendered = rendered_args(super_external_provider_codex_args(
        SuperExternalProvider::Copilot,
        "https://api.githubcopilot.com",
        Some("gpt-5.4"),
        None,
        None,
    ));

    assert!(rendered.contains(&"model_context_window=922000".to_string()));
    assert!(rendered.contains(&"model_auto_compact_token_limit=875900".to_string()));
}
