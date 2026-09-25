use super::*;

#[cfg(feature = "mojo-core")]
#[test]
fn runtime_feature_config_reaches_mojo_kernel() {
    use prodex_mojo_core::launch::{
        RuntimeFeatureConfigInput, RuntimeFeatureWebSearchMode, plan_runtime_feature_config,
    };

    let plan = plan_runtime_feature_config(RuntimeFeatureConfigInput {
        web_search_mode: Some(RuntimeFeatureWebSearchMode::Indexed),
        rollout_budget_limit: Some(100_000),
        rollout_budget_reminders: &[],
        rollout_budget_sampling_weight: None,
        rollout_budget_prefill_weight: None,
        current_time_reminder: false,
        current_time_reminder_interval: None,
        current_time_clock_source: None,
        respect_system_proxy: false,
        no_respect_system_proxy: false,
    })
    .expect("Mojo should plan valid runtime features");

    assert_eq!(
        plan.web_search_mode,
        Some(RuntimeFeatureWebSearchMode::Indexed)
    );
    assert_eq!(plan.rollout_budget_reminders, [75_000, 50_000, 25_000]);
}

#[cfg(feature = "mojo-core")]
fn rendered_os_args(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
#[cfg(feature = "mojo-core")]
fn run_command_renders_codex_runtime_feature_overrides() {
    let command = parse_cli_command_from([
        "prodex",
        "run",
        "--web-search",
        "indexed",
        "--rollout-budget-tokens",
        "100000",
        "--rollout-budget-reminders",
        "75000,50000,25000",
        "--rollout-budget-sampling-weight",
        "1.5",
        "--rollout-budget-prefill-weight",
        "0.25",
        "--current-time-reminder",
        "--current-time-reminder-interval",
        "2",
        "--current-time-clock-source",
        "system",
        "--respect-system-proxy",
        "exec",
        "hello",
    ])
    .expect("run command should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };

    let rendered = rendered_os_args(
        &args
            .codex_args_with_feature_overrides()
            .expect("runtime feature plan should succeed"),
    );
    assert!(rendered.contains(&"web_search=\"indexed\"".to_string()));
    assert!(rendered.contains(&"features.rollout_budget.enabled=true".to_string()));
    assert!(rendered.contains(&"features.rollout_budget.limit_tokens=100000".to_string()));
    assert!(rendered.contains(
        &"features.rollout_budget.reminder_at_remaining_tokens=[75000,50000,25000]".to_string()
    ));
    assert!(rendered.contains(&"features.rollout_budget.sampling_token_weight=1.5".to_string()));
    assert!(rendered.contains(&"features.rollout_budget.prefill_token_weight=0.25".to_string()));
    assert!(rendered.contains(&"features.current_time_reminder.enabled=true".to_string()));
    assert!(rendered.contains(
        &"features.current_time_reminder.reminder_interval_model_requests=2".to_string()
    ));
    assert!(
        rendered.contains(&"features.current_time_reminder.clock_source=\"system\"".to_string())
    );
    assert!(rendered.contains(&"features.respect_system_proxy=true".to_string()));
}

#[test]
#[cfg(feature = "mojo-core")]
fn run_command_renders_respect_system_proxy_disable_override() {
    let command = parse_cli_command_from(["prodex", "run", "--no-respect-system-proxy", "hello"])
        .expect("run command should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };

    let rendered = rendered_os_args(
        &args
            .codex_args_with_feature_overrides()
            .expect("runtime feature plan should succeed"),
    );
    assert!(rendered.contains(&"features.respect_system_proxy=false".to_string()));
}

#[test]
#[cfg(feature = "mojo-core")]
fn rollout_budget_uses_valid_default_reminder_thresholds() {
    let command = parse_cli_command_from([
        "prodex",
        "run",
        "--rollout-budget-tokens",
        "100000",
        "exec",
        "hello",
    ])
    .expect("run command should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };

    let rendered = rendered_os_args(
        &args
            .codex_args_with_feature_overrides()
            .expect("runtime feature plan should succeed"),
    );
    assert!(rendered.contains(
        &"features.rollout_budget.reminder_at_remaining_tokens=[75000,50000,25000]".to_string()
    ));
}

#[test]
#[cfg(feature = "mojo-core")]
fn super_web_search_override_wins_after_provider_defaults() {
    let args = parse_super_as_runtime_tools(&[
        "prodex",
        "s",
        "--provider",
        "gemini",
        "--api-key",
        "test-key",
        "--web-search",
        "cached",
        "exec",
        "review",
    ]);
    let rendered = rendered_codex_args(&args);
    let live_index = rendered
        .iter()
        .position(|arg| arg == "web_search=\"live\"")
        .expect("provider default should be present");
    let cached_index = rendered
        .iter()
        .position(|arg| arg == "web_search=\"cached\"")
        .expect("feature override should be present");
    assert!(cached_index > live_index);
}

#[cfg(not(feature = "mojo-core"))]
#[test]
fn runtime_features_require_mojo_core_but_empty_features_preserve_args() {
    let command = parse_cli_command_from(["prodex", "run", "hello"])
        .expect("run command without runtime features should parse");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        args.codex_args_with_feature_overrides()
            .expect("empty feature arguments should pass through"),
        os_args(&["hello"])
    );

    let command = parse_cli_command_from(["prodex", "run", "--web-search", "indexed", "hello"])
        .expect("runtime feature flags should remain parseable without Mojo");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(
        args.codex_args_with_feature_overrides()
            .expect_err("requested runtime feature planning requires Mojo")
            .to_string(),
        "Codex runtime feature planning failed"
    );
}

#[test]
fn codex_plugin_commands_default_to_managed_run_passthrough() {
    assert!(should_default_cli_invocation_to_run(&os_args(&[
        "prodex", "plugin", "list",
    ])));
    let command = parse_cli_command_from(["prodex", "plugin", "list"])
        .expect("plugin command should parse as run passthrough");
    let Commands::Run(args) = command else {
        panic!("expected run command");
    };
    assert_eq!(args.codex_args, os_args(&["plugin", "list"]));
}
