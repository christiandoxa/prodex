use super::*;

#[test]
fn runtime_launch_dry_run_plan_builds_caveman_placeholder_cleanup() {
    let base_home = PathBuf::from("/tmp/prodex/profiles/main");
    let managed_root = PathBuf::from("/tmp/prodex/profiles");
    let plan = runtime_launch_dry_run_plan(
        OsString::from("codex"),
        &base_home,
        &managed_root,
        None,
        false,
        "prodex-local",
        RuntimeLaunchDryRunChild::Caveman {
            codex_args: vec![OsString::from("exec")],
        },
    );

    let expected_home = managed_root.join(".prodex-overlay-dry-run-from-main");
    assert_eq!(plan.child.codex_home, expected_home);
    assert_eq!(plan.child.args, vec![OsString::from("exec")]);
    assert_eq!(plan.cleanup_paths, vec![expected_home]);
}

#[test]
fn runtime_launch_dry_run_report_redacts_secret_env_and_args() {
    let codex_home = PathBuf::from("/tmp/prodex-home");
    let plan = RuntimeLaunchPlan::new(
        ChildProcessPlan::new(OsString::from("codex"), codex_home.clone())
            .with_args(vec![
                OsString::from("-c"),
                OsString::from("model=\"gpt-5.4\""),
                OsString::from("--config=api_key=\"secret-value\""),
                OsString::from("--api-key"),
                OsString::from("opaque-cli-value"),
                OsString::from("--header"),
                OsString::from("Authorization: Bearer dry-run-bearer-secret-12345"),
                OsString::from("sk-proj-dry-run-secret-123456789"),
            ])
            .with_extra_env(vec![
                ("ANTHROPIC_AUTH_TOKEN", OsString::from("secret-value")),
                (
                    "PRODEX_VISIBLE_BEARER",
                    OsString::from("Bearer dry-run-env-bearer-secret-12345"),
                ),
                ("PRODEX_VISIBLE", OsString::from("1")),
            ]),
    );

    let report = runtime_launch_dry_run_report("run", &codex_home, None, &plan);

    assert!(report.contains("Model: gpt-5.4"));
    assert!(report.contains("ANTHROPIC_AUTH_TOKEN=<redacted>"));
    assert!(report.contains("PRODEX_VISIBLE=1"));
    assert!(report.contains("Bearer <redacted>"));
    assert!(report.contains("sk-proj-<redacted>"));
    assert!(report.contains("<redacted>"));
    assert!(!report.contains("secret-value"));
    assert!(!report.contains("opaque-cli-value"));
    assert!(!report.contains("dry-run-bearer-secret-12345"));
    assert!(!report.contains("dry-run-secret-123456789"));
    assert!(!report.contains("dry-run-env-bearer-secret-12345"));
    assert!(report.contains("Codex/TUI not started"));
}

#[test]
fn runtime_launch_dry_run_report_prefers_cli_model_flag() {
    let codex_home = PathBuf::from("/tmp/prodex-home");
    let plan = RuntimeLaunchPlan::new(
        ChildProcessPlan::new(OsString::from("codex"), codex_home.clone()).with_args(vec![
            OsString::from("--model"),
            OsString::from("gpt-5.3-codex-spark"),
        ]),
    );

    let report = runtime_launch_dry_run_report("run", &codex_home, None, &plan);

    assert!(report.contains("Model: gpt-5.3-codex-spark"));
}

#[test]
fn runtime_launch_dry_run_report_reads_profile_v2_overlay() {
    let codex_home = std::env::temp_dir().join(format!(
        "prodex-runtime-launch-profile-v2-dry-run-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(
        codex_home.join("config.toml"),
        "model_provider = 'openai'\nmodel = 'gpt-5.4'\n",
    )
    .unwrap();
    std::fs::write(
        codex_home.join("local.config.toml"),
        "model_provider = 'prodex-local'\nmodel = 'qwen3-coder'\n",
    )
    .unwrap();
    let plan = RuntimeLaunchPlan::new(
        ChildProcessPlan::new(OsString::from("codex"), codex_home.clone())
            .with_args(vec![OsString::from("--profile-v2=local")]),
    );

    let report = runtime_launch_dry_run_report("run", &codex_home, None, &plan);

    assert!(report.contains("Provider: prodex-local"));
    assert!(report.contains("Model: qwen3-coder"));
    std::fs::remove_dir_all(codex_home).unwrap();
}
