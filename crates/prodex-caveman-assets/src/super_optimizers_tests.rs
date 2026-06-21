use super::{
    SuperOptimizerMemoryConfig, configure_stdio_mcp_server,
    configure_super_optimizer_mcp_servers_with_sources, find_managed_optimizer_command,
    find_optimizer_command, python_venv_root_for_command, render_super_optimizer_awareness,
    token_savior_mcp_env, token_savior_state_dirs_from_prodex_home,
};
use crate::toml_helpers::ensure_child_table;
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::slice::from_ref;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn write_fake_mcp_package_for_venv_command(command: &Path) {
    let venv_root = python_venv_root_for_command(command).expect("command should be in a venv");
    let site_packages = venv_root
        .join("lib")
        .join("python3.13")
        .join("site-packages")
        .join("mcp");
    fs::create_dir_all(&site_packages).expect("fake mcp package should be created");
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

fn write_fake_sqz_mcp(path: &Path) {
    write_fake_executable(
        path,
        "#!/usr/bin/env sh\nprintf '%s\\n' 'Usage: sqz-mcp [--transport stdio|sse]'\n",
    );
}

#[test]
fn stdio_mcp_server_config_adds_missing_entry() {
    let mut table = toml::Table::new();
    configure_stdio_mcp_server(
        &mut table,
        "prodex-sqz",
        PathBuf::from("/tmp/sqz-mcp"),
        &["--transport", "stdio"],
        &[],
    );

    let mcp_servers = table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .expect("mcp servers table should exist");
    let server = mcp_servers
        .get("prodex-sqz")
        .and_then(toml::Value::as_table)
        .expect("sqz server should exist");
    assert_eq!(
        server.get("command").and_then(toml::Value::as_str),
        Some("/tmp/sqz-mcp")
    );
    let args = server
        .get("args")
        .and_then(toml::Value::as_array)
        .expect("sqz args should exist")
        .iter()
        .map(|arg| arg.as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(args, vec!["--transport", "stdio"]);
}

#[test]
fn stdio_mcp_server_config_preserves_existing_entries() {
    let mut table = toml::Table::new();
    let mcp_servers = ensure_child_table(&mut table, "mcp_servers");
    mcp_servers.insert(
        "prodex-sqz".to_string(),
        toml::Value::Table({
            let mut table = toml::Table::new();
            table.insert(
                "command".to_string(),
                toml::Value::String("/custom/sqz-mcp".to_string()),
            );
            table
        }),
    );
    mcp_servers.insert("custom".to_string(), toml::Value::Table(toml::Table::new()));

    configure_stdio_mcp_server(
        &mut table,
        "prodex-sqz",
        PathBuf::from("/tmp/sqz-mcp"),
        &["--transport", "stdio"],
        &[],
    );

    let mcp_servers = table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .expect("mcp servers table should still exist");
    let server = mcp_servers
        .get("prodex-sqz")
        .and_then(toml::Value::as_table)
        .expect("sqz server should still exist");
    assert_eq!(
        server.get("command").and_then(toml::Value::as_str),
        Some("/custom/sqz-mcp")
    );
    assert!(mcp_servers.contains_key("custom"));
}

#[test]
fn stdio_mcp_server_config_merges_env_into_existing_entry() {
    let mut table = toml::Table::new();
    let mcp_servers = ensure_child_table(&mut table, "mcp_servers");
    mcp_servers.insert(
        "prodex-token-savior".to_string(),
        toml::Value::Table({
            let mut table = toml::Table::new();
            table.insert(
                "command".to_string(),
                toml::Value::String("/custom/token-savior".to_string()),
            );
            table.insert(
                "env".to_string(),
                toml::Value::Table({
                    let mut env = toml::Table::new();
                    env.insert(
                        "CUSTOM_ENV".to_string(),
                        toml::Value::String("preserved".to_string()),
                    );
                    env.insert(
                        "TOKEN_SAVIOR_PROFILE".to_string(),
                        toml::Value::String("legacy".to_string()),
                    );
                    env
                }),
            );
            table
        }),
    );

    configure_stdio_mcp_server(
        &mut table,
        "prodex-token-savior",
        PathBuf::from("/tmp/token-savior"),
        &[],
        &[
            ("TOKEN_SAVIOR_PROFILE", "optimized".to_string()),
            ("TOKEN_SAVIOR_CACHE_DIR", "/tmp/prodex/cache".to_string()),
        ],
    );

    let server = table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .and_then(|servers| servers.get("prodex-token-savior"))
        .and_then(toml::Value::as_table)
        .expect("token-savior server should exist");
    assert_eq!(
        server.get("command").and_then(toml::Value::as_str),
        Some("/custom/token-savior")
    );
    let env = server
        .get("env")
        .and_then(toml::Value::as_table)
        .expect("token-savior env should exist");
    assert_eq!(
        env.get("CUSTOM_ENV").and_then(toml::Value::as_str),
        Some("preserved")
    );
    assert_eq!(
        env.get("TOKEN_SAVIOR_PROFILE")
            .and_then(toml::Value::as_str),
        Some("optimized")
    );
    assert_eq!(
        env.get("TOKEN_SAVIOR_CACHE_DIR")
            .and_then(toml::Value::as_str),
        Some("/tmp/prodex/cache")
    );
}

#[test]
fn token_savior_mcp_env_routes_state_under_prodex_home() {
    let prodex_home = PathBuf::from("/tmp/prodex-home");
    let state_dirs = token_savior_state_dirs_from_prodex_home(&prodex_home);
    let env = token_savior_mcp_env("/workspace", Some(&state_dirs));
    let value_for = |key: &str| {
        env.iter()
            .find_map(|(candidate, value)| (*candidate == key).then_some(value.as_str()))
    };

    assert_eq!(value_for("WORKSPACE_ROOTS"), Some("/workspace"));
    assert_eq!(value_for("TOKEN_SAVIOR_NO_WARMUP"), Some("1"));
    assert_eq!(
        value_for("TOKEN_SAVIOR_CACHE_DIR"),
        Some("/tmp/prodex-home/optimizer-state/token-savior/cache")
    );
    assert_eq!(
        value_for("TOKEN_SAVIOR_STATS_DIR"),
        Some("/tmp/prodex-home/optimizer-state/token-savior/stats")
    );
}

#[test]
fn stdio_mcp_server_config_skips_non_table_mcp_servers() {
    let mut table = toml::Table::new();
    table.insert(
        "mcp_servers".to_string(),
        toml::Value::String("invalid".to_string()),
    );

    configure_stdio_mcp_server(
        &mut table,
        "prodex-sqz",
        PathBuf::from("/tmp/sqz-mcp"),
        &["--transport", "stdio"],
        &[],
    );

    assert_eq!(
        table.get("mcp_servers").and_then(toml::Value::as_str),
        Some("invalid")
    );
}

#[test]
fn managed_optimizer_discovery_finds_sqz_workspace_release_binary() {
    let path_root = temp_dir("sqz-path-root");
    let root = temp_dir("sqz-root");
    let path_command = command_path(path_root.join("sqz-mcp"));
    let command = command_path(root.join("sqz/target/release/sqz-mcp"));
    fs::create_dir_all(path_root.as_path()).expect("path root should be created");
    fs::create_dir_all(command.parent().expect("command parent should exist"))
        .expect("command parent should be created");
    write_fake_sqz_mcp(&path_command);
    write_fake_sqz_mcp(&command);

    let found = find_optimizer_command("sqz-mcp", from_ref(&path_root), from_ref(&root));
    assert_eq!(found, Some(command));

    let _ = fs::remove_dir_all(path_root);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn token_savior_discovery_prefers_managed_venv_over_path() {
    let path_root = temp_dir("path-root");
    let optimizer_root = temp_dir("optimizer-root");
    let path_command = command_path(path_root.join("token-savior"));
    let managed_command = command_path(
        optimizer_root
            .join("token-savior")
            .join(".venv")
            .join("bin")
            .join("token-savior"),
    );
    fs::create_dir_all(path_command.parent().expect("path parent should exist"))
        .expect("path parent should be created");
    fs::create_dir_all(
        managed_command
            .parent()
            .expect("managed parent should exist"),
    )
    .expect("managed parent should be created");
    fs::write(&path_command, "").expect("path command should be written");
    fs::write(&managed_command, "").expect("managed command should be written");
    write_fake_mcp_package_for_venv_command(&managed_command);

    assert_eq!(
        find_optimizer_command(
            "token-savior",
            std::slice::from_ref(&path_root),
            std::slice::from_ref(&optimizer_root),
        ),
        Some(managed_command)
    );

    let _ = fs::remove_dir_all(path_root);
    let _ = fs::remove_dir_all(optimizer_root);
}

#[test]
fn token_savior_discovery_skips_managed_venv_without_mcp_and_falls_back_to_path() {
    let path_root = temp_dir("path-root");
    let optimizer_root = temp_dir("optimizer-root");
    let path_command = command_path(path_root.join("token-savior"));
    let managed_command = command_path(
        optimizer_root
            .join("token-savior")
            .join(".venv")
            .join("bin")
            .join("token-savior"),
    );
    fs::create_dir_all(path_command.parent().expect("path parent should exist"))
        .expect("path parent should be created");
    fs::create_dir_all(
        managed_command
            .parent()
            .expect("managed parent should exist"),
    )
    .expect("managed parent should be created");
    fs::write(&path_command, "").expect("path command should be written");
    fs::write(&managed_command, "").expect("managed command should be written");

    assert_eq!(
        find_optimizer_command(
            "token-savior",
            std::slice::from_ref(&path_root),
            std::slice::from_ref(&optimizer_root),
        ),
        Some(path_command)
    );

    let _ = fs::remove_dir_all(path_root);
    let _ = fs::remove_dir_all(optimizer_root);
}

#[test]
fn managed_token_savior_discovery_prefers_venv_binary() {
    let root = temp_dir("token-savior-root");
    let checkout_command = command_path(root.join("token-savior").join("token-savior"));
    let venv_command = command_path(
        root.join("token-savior")
            .join(".venv")
            .join("bin")
            .join("token-savior"),
    );
    fs::create_dir_all(
        checkout_command
            .parent()
            .expect("checkout parent should exist"),
    )
    .expect("checkout parent should be created");
    fs::create_dir_all(venv_command.parent().expect("venv parent should exist"))
        .expect("venv parent should be created");
    fs::write(&checkout_command, "").expect("checkout command should be written");
    fs::write(&venv_command, "").expect("venv command should be written");
    write_fake_mcp_package_for_venv_command(&venv_command);

    assert_eq!(
        find_managed_optimizer_command("token-savior", std::slice::from_ref(&root)),
        Some(venv_command)
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn mcp_config_registers_managed_sqz_when_path_is_empty() {
    let codex_home = temp_dir("codex-home");
    let optimizer_root = temp_dir("optimizer-root");
    let command = command_path(
        optimizer_root
            .join("sqz")
            .join("target")
            .join("release")
            .join("sqz-mcp"),
    );
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    fs::create_dir_all(command.parent().expect("command parent should exist"))
        .expect("command parent should be created");
    write_fake_sqz_mcp(&command);

    configure_super_optimizer_mcp_servers_with_sources(
        &codex_home,
        &[],
        std::slice::from_ref(&optimizer_root),
        Some(&command),
        SuperOptimizerMemoryConfig::default(),
    )
    .expect("super optimizer MCP config should write");

    let config =
        fs::read_to_string(codex_home.join("config.toml")).expect("config.toml should exist");
    let table = toml::from_str::<toml::Value>(&config).expect("config should parse");
    let command_value = table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .and_then(|servers| servers.get("prodex-sqz"))
        .and_then(toml::Value::as_table)
        .and_then(|server| server.get("command"))
        .and_then(toml::Value::as_str);
    let expected_command = command.display().to_string();
    assert_eq!(command_value, Some(expected_command.as_str()));
    let memory_server = table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .and_then(|servers| servers.get("prodex-memory"));
    assert!(
        memory_server.is_none(),
        "prodex-memory should stay disabled unless memory is requested"
    );

    let _ = fs::remove_dir_all(codex_home);
    let _ = fs::remove_dir_all(optimizer_root);
}

#[test]
fn mcp_config_registers_prodex_memory_when_enabled() {
    let codex_home = temp_dir("codex-home-memory");
    fs::create_dir_all(&codex_home).expect("codex home should exist");

    configure_super_optimizer_mcp_servers_with_sources(
        &codex_home,
        &[],
        &[],
        None,
        SuperOptimizerMemoryConfig {
            enabled: true,
            ..Default::default()
        },
    )
    .expect("super optimizer MCP config should write");

    let config =
        fs::read_to_string(codex_home.join("config.toml")).expect("config.toml should exist");
    let table = toml::from_str::<toml::Value>(&config).expect("config should parse");
    let memory_args = table
        .get("mcp_servers")
        .and_then(toml::Value::as_table)
        .and_then(|servers| servers.get("prodex-memory"))
        .and_then(toml::Value::as_table)
        .and_then(|server| server.get("args"))
        .and_then(toml::Value::as_array)
        .expect("prodex-memory args should be registered");
    assert_eq!(
        memory_args,
        &vec![toml::Value::String("__memory-mcp".to_string())]
    );

    let _ = fs::remove_dir_all(codex_home);
}

#[test]
fn super_optimizer_awareness_includes_dynamic_availability() {
    let path_root = temp_dir("awareness-path-root");
    let optimizer_root = temp_dir("awareness-optimizer-root");
    let rtk = command_path(path_root.join("rtk"));
    let sqz = command_path(
        optimizer_root
            .join("sqz")
            .join("target")
            .join("release")
            .join("sqz-mcp"),
    );
    fs::create_dir_all(rtk.parent().expect("rtk parent should exist"))
        .expect("rtk parent should be created");
    fs::create_dir_all(sqz.parent().expect("sqz parent should exist"))
        .expect("sqz parent should be created");
    write_fake_executable(&rtk, "#!/usr/bin/env sh\nexit 0\n");
    write_fake_sqz_mcp(&sqz);

    let awareness = render_super_optimizer_awareness(
        std::slice::from_ref(&path_root),
        std::slice::from_ref(&optimizer_root),
        Some(&sqz),
        true,
        SuperOptimizerMemoryConfig::default(),
    );
    assert!(awareness.contains("## Available Now"));
    assert!(awareness.contains("- rtk: yes"));
    assert!(awareness.contains("- prodex-sqz MCP: yes"));
    assert!(awareness.contains("- prodex-token-savior MCP: no"));
    assert!(awareness.contains("- prodex-memory MCP: disabled"));
    assert!(awareness.contains("- prodex-memory backend: disabled"));
    assert!(awareness.contains("- presidio: enabled"));

    let _ = fs::remove_dir_all(path_root);
    let _ = fs::remove_dir_all(optimizer_root);
}
