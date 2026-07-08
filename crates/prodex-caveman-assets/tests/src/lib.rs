use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!(
        "prodex-caveman-assets-{name}-{}-{stamp}",
        std::process::id()
    ))
}

#[test]
fn embedded_caveman_assets_verify() {
    let verification =
        verify_embedded_caveman_assets().expect("embedded Caveman assets should verify");

    assert!(verification.codex_plugin_files > 0);
    assert!(verification.claude_plugin_files > 0);
    assert!(verification.skill_files > 0);
}

#[test]
fn configure_rtk_codex_home_writes_awareness_and_agents_reference() {
    let dir = temp_dir("rtk");
    configure_rtk_codex_home(&dir).expect("rtk codex home should configure");

    let rtk_md = fs::read_to_string(dir.join("RTK.md")).expect("RTK.md should exist");
    assert!(rtk_md.contains("RTK - Rust Token Killer"));
    assert!(rtk_md.contains("upstream/input side"));
    assert!(rtk_md.contains("before terminal output enters the model context"));
    assert!(rtk_md.contains("Do not wait for the user to ask again"));
    assert!(rtk_md.contains("rtk <cmd>"));
    assert!(rtk_md.contains("wrapper is only a safety net"));
    assert!(rtk_md.contains("rtk gain"));

    let agents = fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md should exist");
    assert_eq!(agents, format!("@{}\n", dir.join("RTK.md").display()));

    configure_rtk_codex_home(&dir).expect("rtk codex home should be idempotent");
    let agents = fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md should exist");
    assert_eq!(agents.matches("@").count(), 1);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn configure_super_optimizer_codex_home_writes_awareness_and_agents_reference() {
    let dir = temp_dir("super-optimizers");
    configure_super_optimizer_codex_home(&dir)
        .expect("super optimizer codex home should configure");

    let awareness = fs::read_to_string(dir.join("SUPER_OPTIMIZERS.md"))
        .expect("SUPER_OPTIMIZERS.md should exist");
    assert!(awareness.contains("sqz"));
    assert!(awareness.contains("Presidio redaction and prodex-memory are opt-in surfaces"));
    assert!(awareness.contains("RTK handles upstream/input command output"));
    assert!(awareness.contains("local optimizer stack"));
    assert!(awareness.contains("auto-wrappers are only a backstop"));
    assert!(awareness.contains("SQZ handles downstream/context reuse"));
    assert!(awareness.contains("prodex-sqz"));
    assert!(awareness.contains("token-savior"));
    assert!(awareness.contains("Locating symbols, callers, dead code, or API changes"));
    assert!(awareness.contains("claw-compactor"));
    assert!(awareness.contains("prodex-claw-compactor-auto"));
    assert!(awareness.contains("- prodex-inspect MCP:"));
    assert!(awareness.contains("- prodex-memory backend: disabled"));
    let config = fs::read_to_string(dir.join("config.toml")).expect("config should exist");
    assert!(config.contains("[mcp_servers.prodex-inspect]"));
    assert!(config.contains("__inspect-mcp"));

    let agents = fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md should exist");
    assert_eq!(
        agents,
        format!("@{}\n", dir.join("SUPER_OPTIMIZERS.md").display())
    );

    configure_super_optimizer_codex_home(&dir)
        .expect("super optimizer codex home should be idempotent");
    let agents = fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md should exist");
    assert_eq!(agents.matches("SUPER_OPTIMIZERS.md").count(), 1);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn prepare_prodex_overlay_home_drops_inherited_codex_apps_cache() {
    let managed = temp_dir("overlay-managed");
    let base = temp_dir("overlay-base");
    fs::create_dir_all(base.join("cache/codex_apps_server_info")).expect("server cache dir");
    fs::create_dir_all(base.join("cache/codex_apps_tools")).expect("tools cache dir");
    fs::create_dir_all(base.join("cache/codex_app_directory")).expect("directory cache dir");
    fs::create_dir_all(base.join("cache/unrelated")).expect("unrelated cache dir");
    fs::write(
        base.join("cache/codex_apps_server_info/server.json"),
        "{\"name\":\"codex-connectors-mcp\"}",
    )
    .expect("server cache");
    fs::write(
        base.join("cache/codex_apps_tools/tools.json"),
        "{\"tools\":[]}",
    )
    .expect("tools cache");
    fs::write(
        base.join("cache/codex_app_directory/apps.json"),
        "{\"apps\":[]}",
    )
    .expect("directory cache");
    fs::write(base.join("cache/unrelated/keep.json"), "{}").expect("unrelated cache");

    let overlay =
        prepare_prodex_overlay_home(&managed, &base).expect("overlay home should be prepared");

    assert!(!overlay.join("cache/codex_apps_server_info").exists());
    assert!(!overlay.join("cache/codex_apps_tools").exists());
    assert!(!overlay.join("cache/codex_app_directory").exists());
    assert!(overlay.join("cache/unrelated/keep.json").is_file());
    assert!(
        base.join("cache/codex_apps_server_info/server.json")
            .is_file()
    );
    assert!(base.join("cache/codex_apps_tools/tools.json").is_file());
    assert!(base.join("cache/codex_app_directory/apps.json").is_file());

    let _ = fs::remove_dir_all(managed);
    let _ = fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn configure_rtk_codex_home_localizes_agents_symlink() {
    let source = temp_dir("rtk-source");
    let overlay = temp_dir("rtk-overlay");
    fs::create_dir_all(&source).expect("source dir");
    fs::create_dir_all(&overlay).expect("overlay dir");
    fs::write(source.join("AGENTS.md"), "# Shared\n").expect("source AGENTS.md");
    std::os::unix::fs::symlink(source.join("AGENTS.md"), overlay.join("AGENTS.md"))
        .expect("agents symlink");

    configure_rtk_codex_home(&overlay).expect("rtk codex home should configure");

    assert!(
        !fs::symlink_metadata(overlay.join("AGENTS.md"))
            .expect("overlay AGENTS metadata")
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(source.join("AGENTS.md")).expect("source AGENTS.md"),
        "# Shared\n"
    );
    let overlay_agents = fs::read_to_string(overlay.join("AGENTS.md")).expect("overlay AGENTS.md");
    assert!(overlay_agents.contains("# Shared"));
    assert!(overlay_agents.contains(&format!("@{}", overlay.join("RTK.md").display())));

    let _ = fs::remove_dir_all(source);
    let _ = fs::remove_dir_all(overlay);
}

#[cfg(unix)]
#[test]
fn configure_super_optimizer_codex_home_localizes_agents_symlink() {
    let source = temp_dir("super-optimizers-source");
    let overlay = temp_dir("super-optimizers-overlay");
    fs::create_dir_all(&source).expect("source dir");
    fs::create_dir_all(&overlay).expect("overlay dir");
    fs::write(source.join("AGENTS.md"), "# Shared\n").expect("source AGENTS.md");
    std::os::unix::fs::symlink(source.join("AGENTS.md"), overlay.join("AGENTS.md"))
        .expect("agents symlink");

    configure_super_optimizer_codex_home(&overlay)
        .expect("super optimizer codex home should configure");

    assert!(
        !fs::symlink_metadata(overlay.join("AGENTS.md"))
            .expect("overlay AGENTS metadata")
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_to_string(source.join("AGENTS.md")).expect("source AGENTS.md"),
        "# Shared\n"
    );
    let overlay_agents = fs::read_to_string(overlay.join("AGENTS.md")).expect("overlay AGENTS.md");
    assert!(overlay_agents.contains("# Shared"));
    assert!(overlay_agents.contains(&format!(
        "@{}",
        overlay.join("SUPER_OPTIMIZERS.md").display()
    )));

    let _ = fs::remove_dir_all(source);
    let _ = fs::remove_dir_all(overlay);
}

#[cfg(unix)]
#[test]
fn configure_rtk_codex_home_replaces_broken_agents_symlink() {
    let overlay = temp_dir("rtk-broken-overlay");
    fs::create_dir_all(&overlay).expect("overlay dir");
    std::os::unix::fs::symlink(overlay.join("missing-AGENTS.md"), overlay.join("AGENTS.md"))
        .expect("broken agents symlink");

    configure_rtk_codex_home(&overlay).expect("rtk codex home should configure");

    assert!(
        !fs::symlink_metadata(overlay.join("AGENTS.md"))
            .expect("overlay AGENTS metadata")
            .file_type()
            .is_symlink()
    );
    let overlay_agents = fs::read_to_string(overlay.join("AGENTS.md")).expect("overlay AGENTS.md");
    assert_eq!(
        overlay_agents,
        format!("@{}\n", overlay.join("RTK.md").display())
    );

    let _ = fs::remove_dir_all(overlay);
}

#[cfg(unix)]
#[test]
fn prepare_caveman_home_then_rtk_handles_broken_agents_symlink() {
    let base = temp_dir("rtk-broken-base");
    let managed_root = temp_dir("rtk-broken-managed");
    fs::create_dir_all(&base).expect("base dir");
    std::os::unix::fs::symlink(base.join("missing-AGENTS.md"), base.join("AGENTS.md"))
        .expect("broken base agents symlink");

    let overlay = prepare_caveman_launch_home(&managed_root, &base)
        .expect("caveman launch home should prepare");
    configure_rtk_codex_home(&overlay).expect("rtk codex home should configure");

    assert!(
        !fs::symlink_metadata(overlay.join("AGENTS.md"))
            .expect("overlay AGENTS metadata")
            .file_type()
            .is_symlink()
    );
    let overlay_agents = fs::read_to_string(overlay.join("AGENTS.md")).expect("overlay AGENTS.md");
    assert_eq!(
        overlay_agents,
        format!("@{}\n", overlay.join("RTK.md").display())
    );

    let _ = fs::remove_dir_all(base);
    let _ = fs::remove_dir_all(managed_root);
}

#[cfg(unix)]
#[test]
fn prepare_caveman_home_handles_broken_config_symlink() {
    let base = temp_dir("broken-config-base");
    let managed_root = temp_dir("broken-config-managed");
    fs::create_dir_all(&base).expect("base dir");
    std::os::unix::fs::symlink(base.join("missing-config.toml"), base.join("config.toml"))
        .expect("broken config symlink");

    let overlay = prepare_caveman_launch_home(&managed_root, &base)
        .expect("caveman launch home should prepare");

    assert!(
        !fs::symlink_metadata(overlay.join("config.toml"))
            .expect("overlay config metadata")
            .file_type()
            .is_symlink()
    );
    let overlay_config =
        fs::read_to_string(overlay.join("config.toml")).expect("overlay config.toml");
    assert!(overlay_config.contains("prodex-caveman"));
    assert!(overlay_config.contains("plugins = false"));
    assert!(overlay_config.contains("remote_plugin = false"));
    let hook_script = fs::read_to_string(overlay.join("bin").join("prodex-caveman-sessionstart"))
        .expect("Caveman SessionStart script should exist");
    assert!(hook_script.contains("CAVEMAN MODE ACTIVE"));
    assert!(hook_script.contains("PRODEX SUPER OPTIMIZERS ACTIVE WHEN AVAILABLE"));
    assert!(hook_script.contains("Ponytail applies smallest-correct-implementation pressure"));
    assert!(hook_script.contains("rtk <cmd>"));
    assert!(hook_script.contains("prodex-sqz"));
    assert!(hook_script.contains("prodex-token-savior"));
    assert!(hook_script.contains("codebase-memory-mcp"));
    assert!(hook_script.contains("prodex-claw-compactor"));
    assert!(hook_script.contains("Presidio is opt-in only"));
    assert!(hook_script.contains(".prodex-hooks/caveman-sessionstart"));

    let _ = fs::remove_dir_all(base);
    let _ = fs::remove_dir_all(managed_root);
}

#[cfg(unix)]
#[test]
fn prepare_caveman_home_shares_base_chat_history() {
    let base = temp_dir("history-base");
    let managed_root = temp_dir("history-managed");
    let base_session_dir = base.join("sessions/2026/05/28");
    fs::create_dir_all(&base_session_dir).expect("base session dir");
    fs::write(
        base.join("history.jsonl"),
        "{\"session_id\":\"base\",\"text\":\"old chat\"}\n",
    )
    .expect("base history");
    fs::write(base_session_dir.join("base.jsonl"), "old session").expect("base session");

    let overlay = prepare_caveman_launch_home(&managed_root, &base)
        .expect("caveman launch home should prepare");

    assert_eq!(
        fs::read_link(overlay.join("history.jsonl")).expect("history should be a symlink"),
        base.join("history.jsonl")
    );
    assert_eq!(
        fs::read_link(overlay.join("attachments")).expect("attachments should be a symlink"),
        base.join("attachments")
    );
    assert_eq!(
        fs::read_link(overlay.join("sessions")).expect("sessions should be a symlink"),
        base.join("sessions")
    );
    assert_eq!(
        fs::read_to_string(overlay.join("history.jsonl")).expect("overlay history"),
        "{\"session_id\":\"base\",\"text\":\"old chat\"}\n"
    );
    fs::write(
        overlay.join("history.jsonl"),
        concat!(
            "{\"session_id\":\"base\",\"text\":\"old chat\"}\n",
            "{\"session_id\":\"base\",\"text\":\"new chat\"}\n"
        ),
    )
    .expect("overlay history write");
    fs::write(overlay.join("sessions/2026/05/28/new.jsonl"), "new session")
        .expect("overlay session write");

    assert!(
        fs::read_to_string(base.join("history.jsonl"))
            .expect("base history should receive overlay write")
            .contains("\"new chat\"")
    );
    assert_eq!(
        fs::read_to_string(base.join("sessions/2026/05/28/new.jsonl"))
            .expect("base session should receive overlay write"),
        "new session"
    );

    let _ = fs::remove_dir_all(managed_root);
    let _ = fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn prepare_caveman_home_keeps_rollout_state_local_to_launch_home() {
    let base = temp_dir("state-base");
    let managed_root = temp_dir("state-managed");
    fs::create_dir_all(&base).expect("base dir");
    fs::write(base.join("state_5.sqlite"), "shared rollout state").expect("state db");
    fs::write(base.join("state_5.sqlite-shm"), "shared rollout shm").expect("state shm");
    fs::write(base.join("state_5.sqlite-wal"), "shared rollout wal").expect("state wal");
    fs::write(base.join("logs_5.sqlite"), "shared logs").expect("logs db");

    let overlay = prepare_caveman_launch_home(&managed_root, &base)
        .expect("caveman launch home should prepare");

    assert!(!overlay.join("state_5.sqlite").exists());
    assert!(!overlay.join("state_5.sqlite-shm").exists());
    assert!(!overlay.join("state_5.sqlite-wal").exists());
    assert_eq!(
        fs::read_to_string(base.join("state_5.sqlite")).expect("base state should remain"),
        "shared rollout state"
    );
    assert_eq!(
        fs::read_to_string(overlay.join("logs_5.sqlite"))
            .expect("non-rollout state remains copied"),
        "shared logs"
    );
    assert_eq!(
        fs::read_link(overlay.join("sessions")).expect("sessions should stay shared"),
        base.join("sessions")
    );

    let _ = fs::remove_dir_all(managed_root);
    let _ = fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn prepare_caveman_home_localizes_shared_rollout_state_symlinks() {
    let base = temp_dir("state-symlink-base");
    let shared = temp_dir("state-symlink-shared");
    let managed_root = temp_dir("state-symlink-managed");
    fs::create_dir_all(&base).expect("base dir");
    fs::create_dir_all(&shared).expect("shared dir");

    let state_db = shared.join("state_5.sqlite");
    let state_shm = shared.join("state_5.sqlite-shm");
    let state_wal = shared.join("state_5.sqlite-wal");
    fs::write(&state_db, "shared rollout state").expect("state db");
    fs::write(&state_shm, "shared rollout shm").expect("state shm");
    fs::write(&state_wal, "shared rollout wal").expect("state wal");
    std::os::unix::fs::symlink(&state_db, base.join("state_5.sqlite")).expect("state db symlink");
    std::os::unix::fs::symlink(&state_shm, base.join("state_5.sqlite-shm"))
        .expect("state shm symlink");
    std::os::unix::fs::symlink(&state_wal, base.join("state_5.sqlite-wal"))
        .expect("state wal symlink");

    let overlay = prepare_caveman_launch_home(&managed_root, &base)
        .expect("caveman launch home should prepare");

    for (name, target) in [
        ("state_5.sqlite", state_db),
        ("state_5.sqlite-shm", state_shm),
        ("state_5.sqlite-wal", state_wal),
    ] {
        let link = overlay.join(name);
        assert!(
            !fs::symlink_metadata(&link)
                .expect("overlay state metadata")
                .file_type()
                .is_symlink(),
            "{name} should be localized from the shared symlink"
        );
        assert!(
            fs::read_to_string(&link)
                .expect("overlay state should read from local copy")
                .starts_with("shared rollout")
        );
        fs::write(&link, format!("local {name}")).expect("overlay state write");
        assert!(
            fs::read_to_string(&target)
                .expect("shared state should stay untouched")
                .starts_with("shared rollout")
        );
    }

    let _ = fs::remove_dir_all(managed_root);
    let _ = fs::remove_dir_all(base);
    let _ = fs::remove_dir_all(shared);
}

#[cfg(unix)]
#[test]
fn caveman_session_start_script_outputs_once_per_launch_home() {
    let codex_home = temp_dir("caveman-sessionstart-once");
    fs::create_dir_all(&codex_home).expect("codex home should exist");

    configure_caveman_launch_home(&codex_home).expect("caveman home should configure");
    configure_caveman_launch_home(&codex_home)
        .expect("caveman home configure should be idempotent");

    let config = fs::read_to_string(codex_home.join("config.toml")).expect("config should read");
    assert_eq!(config.matches("prodex-caveman-sessionstart").count(), 1);
    assert!(
        !config.contains("zsh_path"),
        "Prodex overlay must not override Codex package-managed zsh discovery"
    );

    let script = codex_home.join("bin").join("prodex-caveman-sessionstart");
    let first = std::process::Command::new(&script)
        .env("CODEX_HOME", &codex_home)
        .output()
        .expect("first SessionStart script should run");
    assert!(first.status.success());
    let first_stdout = String::from_utf8(first.stdout).expect("first stdout should be utf8");
    assert!(first_stdout.contains("CAVEMAN MODE ACTIVE"));
    assert!(first_stdout.contains("PRODEX SUPER OPTIMIZERS ACTIVE WHEN AVAILABLE"));
    assert!(first_stdout.contains("Ponytail applies smallest-correct-implementation pressure"));
    assert!(first_stdout.contains("prodex-sqz"));
    assert!(first_stdout.contains("prodex-token-savior"));
    assert!(first_stdout.contains("codebase-memory-mcp"));
    assert!(first_stdout.contains("prodex-claw-compactor"));
    assert!(first_stdout.contains("Presidio is opt-in only"));

    let second = std::process::Command::new(&script)
        .env("CODEX_HOME", &codex_home)
        .output()
        .expect("second SessionStart script should run");
    assert!(second.status.success());
    assert!(
        second.stdout.is_empty(),
        "SessionStart script should not replay after marker"
    );

    let _ = fs::remove_dir_all(codex_home);
}
