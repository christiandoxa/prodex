use super::*;

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
