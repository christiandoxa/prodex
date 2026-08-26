use super::{ResolvedMainAgentConfig, SuperPromptOrder, resolve_super_launch_decisions_with_order};
use crate::{
    AppPaths, ChildProcessPlan, ModelPreferenceSync, codex_cli_config_override_value,
    model_preference_scope,
};
use prodex_cli::SuperArgs;
use std::cell::RefCell;

fn super_args(values: &[&str]) -> SuperArgs {
    let mut argv = vec!["prodex", "s"];
    argv.extend(values.iter().copied());
    let crate::Commands::Super(mut args) =
        crate::parse_cli_command_from(argv).expect("Super command should parse")
    else {
        panic!("expected Super command");
    };
    args.extract_super_overrides_from_codex_args()
        .expect("Super tail should extract");
    args
}

#[test]
fn expose_configuration_prompts_main_agent_before_presidio_and_sub_agents() {
    let mut args = super_args(&["--no-sub-agent"]);
    let calls = RefCell::new(Vec::new());
    let (_, main, sub) = resolve_super_launch_decisions_with_order(
        &mut args,
        true,
        SuperPromptOrder::MainAgentFirst,
        || {
            calls.borrow_mut().push("presidio");
            Ok(false)
        },
        |_| Ok(None),
        |_, locked| {
            assert_eq!(locked, None);
            calls.borrow_mut().push("main");
            Ok(ResolvedMainAgentConfig {
                provider: prodex_provider_core::ProviderId::OpenAi,
                model: Some("main-model".to_string()),
                reasoning_effort: Some("high".to_string()),
                local_url: None,
            })
        },
        |_| panic!("--no-sub-agent must skip the sub-agent picker"),
    )
    .expect("expose configuration should resolve");
    assert_eq!(&*calls.borrow(), &["main", "presidio"]);
    assert_eq!(main.model.as_deref(), Some("main-model"));
    assert_eq!(main.reasoning_effort.as_deref(), Some("high"));
    assert!(sub.is_none());
}

#[test]
fn explicit_expose_provider_still_allows_main_model_and_effort_selection() {
    let mut args = super_args(&["--provider", "copilot", "--no-sub-agent"]);
    let prompted = RefCell::new(false);
    let (_, main, _) = resolve_super_launch_decisions_with_order(
        &mut args,
        true,
        SuperPromptOrder::MainAgentFirst,
        || Ok(false),
        |_| Ok(None),
        |_, locked| {
            assert_eq!(locked, Some(prodex_provider_core::ProviderId::Copilot));
            *prompted.borrow_mut() = true;
            Ok(ResolvedMainAgentConfig {
                provider: prodex_provider_core::ProviderId::Copilot,
                model: Some("copilot-model".to_string()),
                reasoning_effort: Some("high".to_string()),
                local_url: None,
            })
        },
        |_| panic!("--no-sub-agent must skip the sub-agent picker"),
    )
    .expect("explicit provider should resolve");
    assert!(*prompted.borrow());
    assert_eq!(main.model.as_deref(), Some("copilot-model"));
    assert_eq!(main.reasoning_effort.as_deref(), Some("high"));
}

#[test]
fn noninteractive_expose_freezes_explicit_main_pair_without_reading_stdin() {
    let mut args = super_args(&[
        "--model",
        "gpt-5.6-luna",
        "-c",
        "model_reasoning_effort=\"max\"",
        "--no-sub-agent",
    ]);
    let (_, main, sub) = crate::resolve_super_expose_configuration(&mut args, false)
        .expect("explicit noninteractive expose configuration should resolve");
    assert_eq!(main.model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(main.reasoning_effort.as_deref(), Some("max"));
    assert!(sub.is_none());
    assert_eq!(args.local_model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(
        codex_cli_config_override_value(&args.codex_args, "model_reasoning_effort").as_deref(),
        Some("max")
    );
}

#[test]
fn effort_validation_follows_the_selected_model_catalog() {
    assert!(
        super::super_main_prompt::ensure_supported_effort(
            prodex_provider_core::ProviderId::Copilot,
            Some("auto"),
            "high",
        )
        .is_ok()
    );
    assert!(
        super::super_main_prompt::ensure_supported_effort(
            prodex_provider_core::ProviderId::Copilot,
            Some("auto"),
            "max",
        )
        .is_err()
    );
}

#[test]
fn remembered_main_pair_seeds_a_new_expose_configuration_without_locking_it() {
    let root = crate::test_temp_root().join(format!(
        "prodex-expose-preferences-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos()
    ));
    let home = root.join("home");
    std::fs::create_dir_all(&home).expect("test home should be created");
    let _env_lock = crate::test_support::TestEnvVarGuard::lock();
    let _home_guards = crate::test_support::TestEnvVarGuard::set_home(&home);
    let _prodex_home = crate::test_support::TestEnvVarGuard::set(
        "PRODEX_HOME",
        &root.join("prodex").to_string_lossy(),
    );
    let paths = AppPaths::discover().expect("test paths should resolve");
    let codex_home = prodex_core::default_codex_home(&paths).expect("Codex home should resolve");
    std::fs::create_dir_all(&codex_home).expect("Codex home should be created");
    let config_path = codex_home.join("config.toml");
    std::fs::write(
        &config_path,
        "model_provider = \"openai\"\nmodel = \"initial\"\n",
    )
    .expect("initial Codex config should be written");
    let args = super_args(&["--no-sub-agent"]);
    let configured = super::super_main_prompt::resolve_main_model_and_effort(
        &args,
        prodex_provider_core::ProviderId::OpenAi,
        false,
    )
    .expect("current Codex configuration should resolve");
    assert_eq!(configured.0.as_deref(), Some("initial"));
    let scope = model_preference_scope(&codex_home, &[]).expect("preference scope should resolve");
    let child = ChildProcessPlan::new("codex".into(), codex_home.clone());
    let mut sync = ModelPreferenceSync::start_with_scope(&paths, &child, scope)
        .expect("preference sync should start");
    std::fs::write(
        &config_path,
        "model_provider = \"openai\"\nmodel = \"remembered-model\"\nmodel_reasoning_effort = \"high\"\n",
    )
    .expect("remembered Codex config should be written");
    assert!(sync.finish().is_none());

    let args = super_args(&["--no-sub-agent"]);
    let remembered = super::super_main_prompt::resolve_main_model_and_effort(
        &args,
        prodex_provider_core::ProviderId::OpenAi,
        false,
    )
    .expect("remembered expose pair should resolve");
    assert_eq!(remembered.0.as_deref(), Some("remembered-model"));
    assert_eq!(remembered.1.as_deref(), Some("high"));

    let mut explicit = super_args(&[
        "--model",
        "selected-model",
        "-c",
        "model_reasoning_effort=\"max\"",
        "--no-sub-agent",
    ]);
    explicit.extract_super_overrides_from_codex_args().unwrap();
    let selected = super::super_main_prompt::resolve_main_model_and_effort(
        &explicit,
        prodex_provider_core::ProviderId::OpenAi,
        false,
    )
    .expect("explicit expose pair should resolve");
    assert_eq!(selected.0.as_deref(), Some("selected-model"));
    assert_eq!(selected.1.as_deref(), Some("max"));
    let _ = std::fs::remove_dir_all(root);
}
