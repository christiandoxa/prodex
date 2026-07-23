use super::Fixture;
use std::io::Write;
use std::process::{Child, Command, Stdio};

pub(crate) fn run_prodex(fixture: &Fixture, args: &[&str]) -> std::process::Output {
    run_prodex_with_env(fixture, args, &[])
}

pub(crate) fn run_prodex_with_direct_provider(
    fixture: &Fixture,
    args: &[&str],
) -> std::process::Output {
    run_prodex_with_direct_provider_and_env(fixture, args, &[])
}

pub(crate) fn run_prodex_with_direct_provider_and_env(
    fixture: &Fixture,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> std::process::Output {
    let mut direct_args = Vec::with_capacity(args.len() + 2);
    direct_args.extend_from_slice(args);
    direct_args.push("-c");
    direct_args.push("model_provider=\"prodex-test-direct\"");
    run_prodex_with_env(fixture, &direct_args, extra_env)
}

pub(crate) fn run_prodex_with_env(
    fixture: &Fixture,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> std::process::Output {
    let login_id_token = super::chatgpt_id_token("main@example.com");
    let ready_timeout_ms = extra_env
        .iter()
        .find_map(|(key, value)| {
            (*key == "PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS").then_some(*value)
        })
        .unwrap_or("30000");
    let health_connect_timeout_ms = extra_env
        .iter()
        .find_map(|(key, value)| {
            (*key == "PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS").then_some(*value)
        })
        .unwrap_or("1500");
    let health_read_timeout_ms = extra_env
        .iter()
        .find_map(|(key, value)| {
            (*key == "PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS").then_some(*value)
        })
        .unwrap_or("3000");
    Command::new(env!("CARGO_BIN_EXE_prodex"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PRODEX_HOME", &fixture.prodex_home)
        .env("PRODEX_SHARED_CODEX_HOME", &fixture.shared_codex_home)
        .env("PRODEX_CODEX_BIN", &fixture.codex_bin)
        .env("CODEX_CHATGPT_BASE_URL", &fixture.usage_base_url)
        .env("TEST_CODEX_LOG", &fixture.codex_log)
        .env("TEST_LOGIN_ID_TOKEN", login_id_token)
        .env("PRODEX_TEST_SKIP_BINARY_SHA256", "1")
        .env("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", ready_timeout_ms)
        .env(
            "PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS",
            health_connect_timeout_ms,
        )
        .env(
            "PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS",
            health_read_timeout_ms,
        )
        .env("PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS", "250")
        .envs(extra_env.iter().copied())
        .args(args)
        .output()
        .expect("failed to execute prodex")
}

pub(crate) fn run_prodex_with_env_and_stdin(
    fixture: &Fixture,
    args: &[&str],
    extra_env: &[(&str, &str)],
    stdin: &str,
) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PRODEX_HOME", &fixture.prodex_home)
        .env("PRODEX_SHARED_CODEX_HOME", &fixture.shared_codex_home)
        .env("PRODEX_CODEX_BIN", &fixture.codex_bin)
        .env("CODEX_CHATGPT_BASE_URL", &fixture.usage_base_url)
        .env("TEST_CODEX_LOG", &fixture.codex_log)
        .env("PRODEX_TEST_SKIP_BINARY_SHA256", "1")
        .env("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "30000")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS", "1500")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS", "3000")
        .env("PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS", "250")
        .envs(extra_env.iter().copied())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn prodex");

    let mut child_stdin = child.stdin.take().expect("prodex stdin should be piped");
    child_stdin
        .write_all(stdin.as_bytes())
        .expect("failed to write prodex stdin");
    drop(child_stdin);

    child
        .wait_with_output()
        .expect("failed to wait for prodex output")
}

pub(crate) fn spawn_prodex_with_env(
    fixture: &Fixture,
    args: &[&str],
    extra_env: &[(&str, &str)],
) -> Child {
    Command::new(env!("CARGO_BIN_EXE_prodex"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PRODEX_HOME", &fixture.prodex_home)
        .env("PRODEX_SHARED_CODEX_HOME", &fixture.shared_codex_home)
        .env("PRODEX_CODEX_BIN", &fixture.codex_bin)
        .env("CODEX_CHATGPT_BASE_URL", &fixture.usage_base_url)
        .env("TEST_CODEX_LOG", &fixture.codex_log)
        .env("PRODEX_TEST_SKIP_BINARY_SHA256", "1")
        .env("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "30000")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS", "1500")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS", "3000")
        .env("PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS", "250")
        .envs(extra_env.iter().copied())
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn()
        .expect("failed to spawn prodex")
}
