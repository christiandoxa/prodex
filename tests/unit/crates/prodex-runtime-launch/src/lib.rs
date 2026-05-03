use super::*;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

thread_local! {
    static TEST_ENV_LOCK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct TestEnvLockGuard {
    _guard: Option<std::sync::MutexGuard<'static, ()>>,
}

fn acquire_test_env_lock() -> TestEnvLockGuard {
    let guard = TEST_ENV_LOCK_DEPTH.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        if current == 0 {
            Some(
                TEST_ENV_LOCK
                    .get_or_init(|| Mutex::new(()))
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            )
        } else {
            None
        }
    });

    TestEnvLockGuard { _guard: guard }
}

struct TestEnvVarGuard {
    _lock: Option<TestEnvLockGuard>,
    key: Option<&'static str>,
    previous: Option<OsString>,
}

impl TestEnvVarGuard {
    fn lock() -> Self {
        Self {
            _lock: Some(acquire_test_env_lock()),
            key: None,
            previous: None,
        }
    }

    fn set(key: &'static str, value: &str) -> Self {
        let lock = acquire_test_env_lock();
        let previous = env::var_os(key);
        unsafe { env::set_var(key, value) };
        Self {
            _lock: Some(lock),
            key: Some(key),
            previous,
        }
    }

    fn unset(key: &'static str) -> Self {
        let lock = acquire_test_env_lock();
        let previous = env::var_os(key);
        unsafe { env::remove_var(key) };
        Self {
            _lock: Some(lock),
            key: Some(key),
            previous,
        }
    }
}

impl Drop for TestEnvVarGuard {
    fn drop(&mut self) {
        if let Some(key) = self.key {
            if let Some(value) = self.previous.as_ref() {
                unsafe { env::set_var(key, value) };
            } else {
                unsafe { env::remove_var(key) };
            }
        }
    }
}

fn test_profile(path: &str) -> prodex_state::ProfileEntry {
    prodex_state::ProfileEntry {
        codex_home: PathBuf::from(path),
        managed: true,
        email: None,
        provider: prodex_state::ProfileProvider::Openai,
    }
}

#[test]
fn codex_sandbox_removed_env_strips_inherited_codex_sandbox_vars() {
    let _env_guard = TestEnvVarGuard::lock();
    let _sandbox_guard = TestEnvVarGuard::set("CODEX_SANDBOX", "workspace-write");
    let _network_guard = TestEnvVarGuard::set("CODEX_SANDBOX_NETWORK_DISABLED", "1");
    let _custom_guard = TestEnvVarGuard::set("CODEX_SANDBOX_PROFILE", "danger-full-access");
    let _other_guard = TestEnvVarGuard::set("PRODEX_TEST_KEEP_ENV", "1");

    let removed = codex_sandbox_removed_env();

    assert!(removed.iter().any(|key| key == "CODEX_SANDBOX"));
    assert!(
        removed
            .iter()
            .any(|key| key == "CODEX_SANDBOX_NETWORK_DISABLED")
    );
    assert!(removed.iter().any(|key| key == "CODEX_SANDBOX_PROFILE"));
    assert!(!removed.iter().any(|key| key == "PRODEX_TEST_KEEP_ENV"));
}

#[test]
fn codex_child_plan_applies_codex_sandbox_removed_env() {
    let _env_guard = TestEnvVarGuard::lock();
    let _custom_guard = TestEnvVarGuard::set("CODEX_SANDBOX_PROFILE", "danger-full-access");
    let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
    let _lower_no_proxy_guard = TestEnvVarGuard::set("no_proxy", "internal.local");
    let binary = OsString::from("codex");
    let codex_home = PathBuf::from("/tmp/prodex-codex-home");
    let args = vec![OsString::from("login")];

    let plan = codex_child_plan(
        binary.clone(),
        codex_home.clone(),
        args.clone(),
        "prodex-local",
    );

    assert_eq!(plan.binary, binary);
    assert_eq!(plan.codex_home, codex_home);
    assert_eq!(plan.args, args);
    assert!(plan.removed_env.iter().any(|key| key == "CODEX_SANDBOX"));
    assert!(
        plan.removed_env
            .iter()
            .any(|key| key == "CODEX_SANDBOX_NETWORK_DISABLED")
    );
    assert!(
        plan.removed_env
            .iter()
            .any(|key| key == "CODEX_SANDBOX_PROFILE")
    );
    assert_eq!(
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "NO_PROXY")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("example.com,internal.local,127.0.0.1,localhost,::1".to_string())
    );
    assert_eq!(
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "no_proxy")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("example.com,internal.local,127.0.0.1,localhost,::1".to_string())
    );
}

#[test]
fn local_proxy_bypass_env_deduplicates_existing_values() {
    let _env_guard = TestEnvVarGuard::lock();
    let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", " example.com,127.0.0.1 ");
    let _lower_no_proxy_guard = TestEnvVarGuard::set("no_proxy", "LOCALHOST,internal.local");

    let env = local_proxy_bypass_env();

    assert_eq!(
        env,
        vec![
            (
                "NO_PROXY",
                OsString::from("example.com,127.0.0.1,LOCALHOST,internal.local,::1")
            ),
            (
                "no_proxy",
                OsString::from("example.com,127.0.0.1,LOCALHOST,internal.local,::1")
            )
        ]
    );
}

#[test]
fn codex_child_plan_adds_local_provider_host_to_proxy_bypass_env() {
    let _env_guard = TestEnvVarGuard::lock();
    let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
    let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
    let args = vec![
        OsString::from("-c"),
        OsString::from(
            "model_providers.prodex-local.base_url=\"http://host.docker.internal:11434/v1\"",
        ),
    ];

    let plan = codex_child_plan(
        OsString::from("codex"),
        PathBuf::from("/tmp/prodex-codex-home"),
        args,
        "prodex-local",
    );

    assert_eq!(
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "NO_PROXY")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(
            "example.com,127.0.0.1,localhost,::1,host.docker.internal,host.docker.internal:11434"
                .to_string()
        )
    );
    assert_eq!(
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "no_proxy")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some(
            "example.com,127.0.0.1,localhost,::1,host.docker.internal,host.docker.internal:11434"
                .to_string()
        )
    );
}

#[test]
fn codex_child_plan_adds_runtime_proxy_ports_to_proxy_bypass_env() {
    let _env_guard = TestEnvVarGuard::lock();
    let _http_proxy_guard = TestEnvVarGuard::set("http_proxy", "http://127.0.0.1:1086");
    let _https_proxy_guard = TestEnvVarGuard::set("https_proxy", "http://127.0.0.1:1086");
    let _no_proxy_guard = TestEnvVarGuard::unset("NO_PROXY");
    let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
    let args = vec![
        OsString::from("-c"),
        OsString::from("chatgpt_base_url=\"http://127.0.0.1:64550/backend-api\""),
        OsString::from("-c"),
        OsString::from("openai_base_url=\"http://127.0.0.1:64550/backend-api/prodex\""),
    ];

    let plan = codex_child_plan(
        OsString::from("codex"),
        PathBuf::from("/tmp/prodex-codex-home"),
        args,
        "prodex-local",
    );

    assert_eq!(
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "NO_PROXY")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("127.0.0.1,localhost,::1,127.0.0.1:64550".to_string())
    );
    assert_eq!(
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "no_proxy")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("127.0.0.1,localhost,::1,127.0.0.1:64550".to_string())
    );
}

#[test]
fn remove_upstream_proxy_env_preserves_local_proxy_bypass_env() {
    let _env_guard = TestEnvVarGuard::lock();
    let _http_proxy_guard = TestEnvVarGuard::set("HTTP_PROXY", "http://127.0.0.1:1086");
    let _https_proxy_guard = TestEnvVarGuard::set("https_proxy", "http://127.0.0.1:1086");
    let _no_proxy_guard = TestEnvVarGuard::set("NO_PROXY", "example.com");
    let _lower_no_proxy_guard = TestEnvVarGuard::unset("no_proxy");
    let mut plan = codex_child_plan(
        OsString::from("codex"),
        PathBuf::from("/tmp/prodex-codex-home"),
        vec![],
        "prodex-local",
    );

    remove_upstream_proxy_env(&mut plan);

    assert!(plan.removed_env.iter().any(|key| key == "HTTP_PROXY"));
    assert!(plan.removed_env.iter().any(|key| key == "https_proxy"));
    assert_eq!(
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "NO_PROXY")
            .map(|(_, value)| value.to_string_lossy().into_owned()),
        Some("example.com,127.0.0.1,localhost,::1".to_string())
    );
}

#[test]
fn runtime_proxy_codex_args_preserve_user_overrides_after_proxy_overrides() {
    let args = runtime_proxy_codex_args(
        "127.0.0.1:4455".parse().expect("socket addr"),
        &[
            OsString::from("exec"),
            OsString::from("-c"),
            OsString::from("service_tier=null"),
            OsString::from("--config=notice.fast_default_opt_out=true"),
            OsString::from("hello"),
        ],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert_eq!(args[0], "-c");
    assert_eq!(
        args[1],
        "chatgpt_base_url=\"http://127.0.0.1:4455/backend-api\""
    );
    assert_eq!(args[2], "-c");
    assert_eq!(
        args[3],
        format!(
            "openai_base_url=\"http://127.0.0.1:4455{}\"",
            runtime_proxy::RUNTIME_PROXY_OPENAI_MOUNT_PATH
        )
    );
    assert_eq!(
        &args[4..],
        [
            "exec",
            "-c",
            "service_tier=null",
            "--config=notice.fast_default_opt_out=true",
            "hello"
        ]
    );
}

#[test]
fn prepare_codex_launch_args_extracts_full_access_and_normalizes_resume() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("--full-access"),
            OsString::from("review"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

#[test]
fn prepare_codex_launch_args_extracts_prodex_full_access_passthrough_marker() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("exec"),
            OsString::from("--full-access"),
            OsString::from("review"),
        ],
        false,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("exec"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

#[test]
fn prepare_codex_launch_args_full_access_keeps_resume_normalization_and_review_detection() {
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ],
        true,
    );

    assert_eq!(
        args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("resume"),
            OsString::from("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9"),
            OsString::from("review"),
        ]
    );
    assert!(include_code_review);
}

#[test]
fn profileless_local_home_requires_no_requested_profile_and_matching_provider() {
    assert!(allow_profileless_local_home(
        None,
        Some("PRODEX-LOCAL"),
        "prodex-local"
    ));
    assert!(!allow_profileless_local_home(
        Some("main"),
        Some("prodex-local"),
        "prodex-local"
    ));
    assert!(!allow_profileless_local_home(
        None,
        Some("openai"),
        "prodex-local"
    ));
    assert!(!allow_profileless_local_home(None, None, "prodex-local"));
}

#[test]
fn resolve_profile_name_prefers_requested_then_active_then_single_profile() {
    let state = prodex_state::AppState {
        active_profile: Some("main".to_string()),
        profiles: BTreeMap::from([
            ("backup".to_string(), test_profile("/tmp/backup")),
            ("main".to_string(), test_profile("/tmp/main")),
        ]),
        ..prodex_state::AppState::default()
    };

    assert_eq!(
        resolve_profile_name(&state, Some("backup")).expect("requested profile"),
        "backup"
    );
    assert_eq!(
        resolve_profile_name(&state, None).expect("active profile"),
        "main"
    );

    let single = prodex_state::AppState {
        profiles: BTreeMap::from([("only".to_string(), test_profile("/tmp/only"))]),
        ..prodex_state::AppState::default()
    };
    assert_eq!(
        resolve_profile_name(&single, None).expect("single profile"),
        "only"
    );
}

#[test]
fn record_run_selection_prunes_missing_profiles_before_insert() {
    let mut state = prodex_state::AppState {
        profiles: BTreeMap::from([("main".to_string(), test_profile("/tmp/main"))]),
        last_run_selected_at: BTreeMap::from([
            ("deleted".to_string(), 10),
            ("main".to_string(), 20),
        ]),
        ..prodex_state::AppState::default()
    };

    record_run_selection_at(&mut state, "main", 30);

    assert_eq!(
        state.last_run_selected_at,
        BTreeMap::from([("main".to_string(), 30)])
    );
}

#[test]
fn fixed_runtime_proxy_state_keeps_only_selected_profile() {
    let state = prodex_state::AppState {
        active_profile: Some("second".to_string()),
        profiles: BTreeMap::from([
            ("main".to_string(), test_profile("/tmp/main-home")),
            ("second".to_string(), test_profile("/tmp/second-home")),
        ]),
        last_run_selected_at: BTreeMap::from([
            ("main".to_string(), 1_000),
            ("second".to_string(), 2_000),
        ]),
        response_profile_bindings: BTreeMap::from([
            (
                "resp-main".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: 1_000,
                },
            ),
            (
                "resp-second".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 2_000,
                },
            ),
        ]),
        session_profile_bindings: BTreeMap::from([
            (
                "session-main".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "main".to_string(),
                    bound_at: 1_000,
                },
            ),
            (
                "session-second".to_string(),
                prodex_state::ResponseProfileBinding {
                    profile_name: "second".to_string(),
                    bound_at: 2_000,
                },
            ),
        ]),
    };

    let fixed = fixed_runtime_proxy_state(&state, "main").unwrap();

    assert_eq!(fixed.active_profile.as_deref(), Some("main"));
    assert_eq!(
        fixed
            .profiles
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["main"]
    );
    assert_eq!(
        fixed
            .last_run_selected_at
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["main"]
    );
    assert_eq!(
        fixed
            .response_profile_bindings
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["resp-main"]
    );
    assert_eq!(
        fixed
            .session_profile_bindings
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["session-main"]
    );
}

#[test]
fn ensure_profile_path_is_unique_rejects_existing_profile_home() {
    let state = prodex_state::AppState {
        profiles: BTreeMap::from([("main".to_string(), test_profile("/tmp/main"))]),
        ..prodex_state::AppState::default()
    };

    let err = ensure_profile_path_is_unique(&state, Path::new("/tmp/main"))
        .expect_err("duplicate profile path should fail");

    assert!(err.to_string().contains("already used by profile 'main'"));
}

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

    let expected_home = managed_root.join(".caveman-dry-run-from-main");
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
fn no_ready_runtime_profiles_plan_formats_blocked_report() {
    let report = prodex_shared_types::RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: prodex_quota::AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(prodex_quota::UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(prodex_quota::WindowPair {
                primary_window: Some(prodex_quota::UsageWindow {
                    used_percent: Some(100),
                    reset_at: Some(1_900_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            additional_rate_limits: Vec::new(),
        }),
    };

    let RuntimeLaunchNoReadyProfilesPlan::Blocked {
        blocked_message,
        no_ready_message,
        inspect_hint,
        error_message,
    } = no_ready_runtime_profiles_plan(&report, "main", false)
    else {
        panic!("expected blocked plan");
    };

    assert!(blocked_message.contains("Quota preflight blocked profile 'main'"));
    assert_eq!(no_ready_message, "No ready profile was found.");
    assert!(inspect_hint.contains("prodex quota --profile main"));
    assert!(error_message.contains("no ready profile"));
}

#[test]
fn blocked_selected_runtime_profile_plan_formats_rotation() {
    let blocked = vec![prodex_quota::BlockedLimit {
        message: "5h exhausted until later".to_string(),
    }];
    let plan =
        blocked_selected_runtime_profile_plan("main", &blocked, true, &["backup".to_string()]);

    assert_eq!(
        plan,
        RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
            blocked_message: "Quota preflight blocked profile 'main': 5h exhausted until later"
                .to_string(),
            next_profile: "backup".to_string(),
            rotate_message: "Auto-rotating to profile 'backup'.".to_string(),
        }
    );
}
