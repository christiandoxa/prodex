use super::*;

fn environment(entries: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    entries
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

#[test]
fn codex_sandbox_removed_env_strips_inherited_codex_sandbox_vars() {
    let removed = codex_sandbox_removed_env_from(&environment(&[
        ("CODEX_SANDBOX", "workspace-write"),
        ("CODEX_SANDBOX_NETWORK_DISABLED", "1"),
        ("CODEX_SANDBOX_PROFILE", "danger-full-access"),
        ("PRODEX_TEST_KEEP_ENV", "1"),
    ]));

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
    let environment = environment(&[
        ("CODEX_SANDBOX_PROFILE", "danger-full-access"),
        ("LD_PRELOAD", "/tmp/inject.so"),
        ("DYLD_INSERT_LIBRARIES", "/tmp/inject.dylib"),
        ("NO_PROXY", "example.com"),
        ("no_proxy", "internal.local"),
    ]);
    let binary = OsString::from("codex");
    let codex_home = PathBuf::from("/tmp/prodex-codex-home");
    let args = vec![OsString::from("login")];

    let plan = codex_child_plan_with_env(
        binary.clone(),
        codex_home.clone(),
        args.clone(),
        "prodex-local",
        &environment,
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
    assert!(plan.removed_env.iter().any(|key| key == "LD_PRELOAD"));
    assert!(
        plan.removed_env
            .iter()
            .any(|key| key == "DYLD_INSERT_LIBRARIES")
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
fn codex_child_plan_disables_keyboard_enhancement_only_for_vte_without_override() {
    let plan = |environment: Vec<(OsString, OsString)>| {
        codex_child_plan_with_env(
            OsString::from("codex"),
            PathBuf::from("/tmp/prodex-codex-home"),
            Vec::new(),
            "prodex-local",
            &environment,
        )
    };
    let keyboard_override = |plan: &ChildProcessPlan| {
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "CODEX_TUI_DISABLE_KEYBOARD_ENHANCEMENT")
            .map(|(_, value)| value.to_string_lossy().into_owned())
    };

    assert_eq!(
        keyboard_override(&plan(environment(&[("VTE_VERSION", "7600")]))),
        Some("1".to_string())
    );
    assert_eq!(keyboard_override(&plan(environment(&[]))), None);
    assert_eq!(
        keyboard_override(&plan(environment(&[
            ("VTE_VERSION", "7600"),
            ("CODEX_TUI_DISABLE_KEYBOARD_ENHANCEMENT", "0"),
        ]))),
        None
    );
}

#[test]
fn child_process_hardening_strips_dynamic_loader_env_unless_allowed() {
    let removed = child_process_hardening_removed_env_from(&environment(&[
        ("LD_PRELOAD", "/tmp/inject.so"),
        ("DYLD_CUSTOM_TEST", "1"),
    ]));

    assert!(removed.iter().any(|key| key == "LD_PRELOAD"));
    assert!(removed.iter().any(|key| key == "LD_LIBRARY_PATH"));
    assert!(removed.iter().any(|key| key == "DYLD_CUSTOM_TEST"));
}

#[test]
fn child_process_hardening_can_be_explicitly_disabled() {
    assert!(
        child_process_hardening_removed_env_from(&environment(&[
            ("LD_PRELOAD", "/tmp/inject.so"),
            ("PRODEX_ALLOW_UNSAFE_CHILD_ENV", "1"),
        ]))
        .is_empty()
    );
}

#[test]
fn codex_child_plan_scopes_pre_exec_config_overrides() {
    let args = vec![
        OsString::from("-c"),
        OsString::from("model_provider=\"prodex-local\""),
        OsString::from("-c"),
        OsString::from("model_providers.prodex-local.base_url=\"http://127.0.0.1:4455/v1\""),
        OsString::from("--dangerously-bypass-approvals-and-sandbox"),
        OsString::from("exec"),
        OsString::from("--json"),
        OsString::from("hello"),
    ];

    let plan = codex_child_plan(
        OsString::from("codex"),
        PathBuf::from("/tmp/prodex-codex-home"),
        args,
        "prodex-local",
    );

    assert_eq!(
        plan.args,
        vec![
            OsString::from("--dangerously-bypass-approvals-and-sandbox"),
            OsString::from("exec"),
            OsString::from("-c"),
            OsString::from("model_provider=\"prodex-local\""),
            OsString::from("-c"),
            OsString::from("model_providers.prodex-local.base_url=\"http://127.0.0.1:4455/v1\""),
            OsString::from("--json"),
            OsString::from("hello"),
        ]
    );
    assert!(
        plan.extra_env
            .iter()
            .find(|(key, _)| key == "NO_PROXY")
            .map(|(_, value)| value.to_string_lossy().contains("127.0.0.1:4455"))
            .unwrap_or(false)
    );
}

#[test]
fn local_proxy_bypass_env_deduplicates_existing_values() {
    let env = local_proxy_bypass_env_for_hosts_and_env(
        std::iter::empty::<&str>(),
        &environment(&[
            ("NO_PROXY", " example.com,127.0.0.1 "),
            ("no_proxy", "LOCALHOST,internal.local"),
        ]),
    );

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
    let args = vec![
        OsString::from("-c"),
        OsString::from(
            "model_providers.prodex-local.base_url=\"http://host.docker.internal:11434/v1\"",
        ),
    ];

    let plan = codex_child_plan_with_env(
        OsString::from("codex"),
        PathBuf::from("/tmp/prodex-codex-home"),
        args,
        "prodex-local",
        &environment(&[("NO_PROXY", "example.com")]),
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
    let args = vec![
        OsString::from("-c"),
        OsString::from("chatgpt_base_url=\"http://127.0.0.1:64550/backend-api\""),
        OsString::from("-c"),
        OsString::from("openai_base_url=\"http://127.0.0.1:64550/backend-api/prodex\""),
    ];

    let plan = codex_child_plan_with_env(
        OsString::from("codex"),
        PathBuf::from("/tmp/prodex-codex-home"),
        args,
        "prodex-local",
        &environment(&[
            ("http_proxy", "http://127.0.0.1:1086"),
            ("https_proxy", "http://127.0.0.1:1086"),
        ]),
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
    let mut plan = codex_child_plan_with_env(
        OsString::from("codex"),
        PathBuf::from("/tmp/prodex-codex-home"),
        vec![],
        "prodex-local",
        &environment(&[
            ("HTTP_PROXY", "http://127.0.0.1:1086"),
            ("https_proxy", "http://127.0.0.1:1086"),
            ("NO_PROXY", "example.com"),
        ]),
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
