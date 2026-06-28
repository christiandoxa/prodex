use super::{
    SuperOptimizerMemoryConfig, codebase_memory_mcp_env,
    configure_super_optimizer_mcp_servers_with_sources, find_optimizer_command,
};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_ENV_LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &Path) -> Self {
        let lock = env_lock().lock().expect("env lock should not be poisoned");
        let previous = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }
        Self {
            key,
            previous,
            _lock: lock,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe { env::set_var(self.key, value) },
            None => unsafe { env::remove_var(self.key) },
        }
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!(
        "prodex-super-optimizers-{name}-{}-{stamp}",
        std::process::id()
    ))
}

fn command_path(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    if path.extension().is_none() {
        return path.with_extension("exe");
    }
    path
}

fn write_fake_executable(path: &Path, script: &str) {
    fs::write(path, script).expect("fake executable should be written");
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path)
            .expect("fake executable should stat")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("fake executable should chmod");
    }
}

fn write_fake_codebase_memory_mcp(path: &Path) {
    write_fake_executable(
        path,
        r#"#!/usr/bin/env sh
case "$*" in
  *--help*) printf '%s\n' 'Usage: codebase-memory-mcp'; exit 0 ;;
esac
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*) printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"tools":{"listChanged":false}},"protocolVersion":"2024-11-05","serverInfo":{"name":"codebase-memory-mcp","version":"test"}}}' ;;
    *'"method":"tools/list"'*) printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"search_graph","inputSchema":{"type":"object"}}]}}' ;;
  esac
done
"#,
    );
}

#[test]
fn codebase_memory_mcp_env_routes_cache_under_prodex_home() {
    let prodex_home = temp_dir("prodex-home-cbm");
    let _env = EnvGuard::set("PRODEX_HOME", &prodex_home);

    let env = codebase_memory_mcp_env();

    assert_eq!(
        env.iter()
            .find_map(|(key, value)| (*key == "CBM_CACHE_DIR").then_some(value.as_str())),
        Some(
            prodex_home
                .join("optimizer-state/codebase-memory/cache")
                .to_str()
                .unwrap()
        )
    );

    let _ = fs::remove_dir_all(prodex_home);
}

#[test]
fn codebase_memory_discovery_finds_managed_build_binary() {
    let optimizer_root = temp_dir("cbm-root");
    let command = command_path(
        optimizer_root
            .join("codebase-memory-mcp")
            .join("build")
            .join("c")
            .join("codebase-memory-mcp"),
    );
    fs::create_dir_all(command.parent().expect("command parent should exist"))
        .expect("command parent should be created");
    write_fake_codebase_memory_mcp(&command);

    assert_eq!(
        find_optimizer_command(
            "codebase-memory-mcp",
            &[],
            std::slice::from_ref(&optimizer_root),
        ),
        Some(command)
    );

    let _ = fs::remove_dir_all(optimizer_root);
}

#[test]
fn mcp_config_registers_codebase_memory_when_available() {
    let codex_home = temp_dir("codex-home-cbm");
    let optimizer_root = temp_dir("optimizer-root-cbm");
    let command = command_path(
        optimizer_root
            .join("codebase-memory-mcp")
            .join("build")
            .join("c")
            .join("codebase-memory-mcp"),
    );
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    fs::create_dir_all(command.parent().expect("command parent should exist"))
        .expect("command parent should be created");
    write_fake_codebase_memory_mcp(&command);

    configure_super_optimizer_mcp_servers_with_sources(
        &codex_home,
        &[],
        std::slice::from_ref(&optimizer_root),
        None,
        Some(&command),
        SuperOptimizerMemoryConfig::default(),
    )
    .expect("super optimizer MCP config should write");

    let config =
        fs::read_to_string(codex_home.join("config.toml")).expect("config.toml should exist");
    let table = toml::from_str::<toml::Value>(&config).expect("config should parse");
    let server = table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .and_then(|servers| servers.get("codebase-memory-mcp"))
        .and_then(toml::Value::as_table)
        .expect("codebase-memory-mcp should be registered");
    assert_eq!(
        server.get("command").and_then(toml::Value::as_str),
        Some(env::current_exe().unwrap().to_str().unwrap())
    );
    let args = server
        .get("args")
        .and_then(toml::Value::as_array)
        .expect("bridge args should be registered")
        .iter()
        .map(|arg| arg.as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(args, vec!["__mcp-jsonl-bridge", command.to_str().unwrap()]);
    assert!(
        server
            .get("env")
            .and_then(toml::Value::as_table)
            .and_then(|env| env.get("CBM_CACHE_DIR"))
            .and_then(toml::Value::as_str)
            .expect("CBM_CACHE_DIR should be set")
            .contains("optimizer-state/codebase-memory/cache")
    );

    let _ = fs::remove_dir_all(codex_home);
    let _ = fs::remove_dir_all(optimizer_root);
}
