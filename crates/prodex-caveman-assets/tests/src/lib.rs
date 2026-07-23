use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir()
        .canonicalize()
        .expect("temp dir should resolve")
        .join(format!(
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
    assert!(awareness.contains("Use visible `rtk <cmd>`"));
    assert!(awareness.contains("codebase-memory-mcp"));
    assert!(awareness.contains("playwright-mcp"));
    assert!(awareness.contains("Ponytail"));
    for removed in [
        "prodex-sqz",
        "token-savior",
        "claw-compactor",
        "prodex-inspect",
        "prodex-memory",
    ] {
        assert!(!awareness.contains(removed));
    }
    let config = fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
    assert!(!config.contains("prodex-inspect"));

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

#[test]
fn prepare_runtime_overlay_home_does_not_install_caveman_assets() {
    let managed = temp_dir("runtime-overlay-managed");
    let base = temp_dir("runtime-overlay-base");
    fs::create_dir_all(&base).expect("base home");
    fs::write(base.join("config.toml"), "model = 'gpt-5'\n").expect("base config");

    let overlay =
        prepare_runtime_overlay_home(&managed, &base).expect("runtime overlay should be prepared");

    assert_eq!(
        fs::read_to_string(overlay.join("config.toml")).expect("overlay config"),
        "model = 'gpt-5'\n"
    );
    assert!(!overlay.join(".tmp/marketplaces/prodex-caveman").exists());
    assert!(
        !overlay
            .join("plugins/cache/prodex-caveman/caveman")
            .exists()
    );

    let _ = fs::remove_dir_all(managed);
    let _ = fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn prepare_prodex_overlay_home_rejects_symlink_managed_root() {
    let root = temp_dir("overlay-symlink-root");
    let managed = root.join("managed");
    let outside = root.join("outside");
    let base = root.join("base");
    fs::create_dir_all(&outside).expect("outside root should exist");
    fs::create_dir_all(&base).expect("base should exist");
    std::os::unix::fs::symlink(&outside, &managed).expect("managed symlink should be created");

    let err = prepare_prodex_overlay_home(&managed, &base)
        .expect_err("symlink managed root should be rejected");

    assert!(
        err.to_string().contains("must not be a symbolic link"),
        "unexpected error: {err:#}"
    );
    assert!(
        fs::read_dir(&outside)
            .expect("outside target should be readable")
            .next()
            .is_none(),
        "overlay creation must not write through symlinked managed root"
    );
    let _ = fs::remove_dir_all(root);
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
    assert!(overlay_config.contains("developer_instructions"));
    assert!(overlay_config.contains("CAVEMAN MODE ACTIVE"));
    assert!(overlay_config.contains("PRODEX SUPER TOOLS ACTIVE WHEN AVAILABLE"));
    assert!(!overlay_config.contains("hooks.SessionStart"));

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
fn prepare_caveman_home_shares_rollout_state_with_base() {
    let base = temp_dir("state-base");
    let managed_root = temp_dir("state-managed");
    fs::create_dir_all(&base).expect("base dir");
    fs::write(base.join("state_5.sqlite"), "shared rollout state").expect("state db");
    fs::write(base.join("state_5.sqlite-shm"), "shared rollout shm").expect("state shm");
    fs::write(base.join("state_5.sqlite-wal"), "shared rollout wal").expect("state wal");
    fs::write(base.join("logs_5.sqlite"), "shared logs").expect("logs db");

    let overlay = prepare_caveman_launch_home(&managed_root, &base)
        .expect("caveman launch home should prepare");

    for name in ["state_5.sqlite", "state_5.sqlite-shm", "state_5.sqlite-wal"] {
        let link = overlay.join(name);
        assert!(
            fs::symlink_metadata(&link)
                .expect("overlay state metadata")
                .file_type()
                .is_symlink(),
            "{name} should stay shared with the base profile"
        );
        fs::write(&link, format!("updated {name}")).expect("overlay state write should reach base");
        assert_eq!(
            fs::read_to_string(base.join(name)).expect("base state should persist update"),
            format!("updated {name}")
        );
    }
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
fn prepare_runtime_home_localizes_shared_rollout_state_symlinks() {
    let base = temp_dir("state-symlink-base");
    let shared = temp_dir("state-symlink-shared");
    let managed_root = temp_dir("state-symlink-managed");
    fs::create_dir_all(&base).expect("base dir");
    fs::create_dir_all(&shared).expect("shared dir");

    let state_db = shared.join("state_5.sqlite");
    let state_shm = shared.join("state_5.sqlite-shm");
    let state_wal = shared.join("state_5.sqlite-wal");
    let mut state_db_contents = b"shared rollout state".to_vec();
    state_db_contents.extend(vec![b'x'; 600 * 1024]);
    let state_db_len = state_db_contents.len() as u64;
    fs::write(&state_db, &state_db_contents).expect("state db");
    fs::write(&state_shm, "shared rollout shm").expect("state shm");
    fs::write(&state_wal, "shared rollout wal").expect("state wal");
    std::os::unix::fs::symlink(&state_db, base.join("state_5.sqlite")).expect("state db symlink");
    std::os::unix::fs::symlink(&state_shm, base.join("state_5.sqlite-shm"))
        .expect("state shm symlink");
    std::os::unix::fs::symlink(&state_wal, base.join("state_5.sqlite-wal"))
        .expect("state wal symlink");

    let overlay = prepare_runtime_overlay_home(&managed_root, &base)
        .expect("runtime launch home should prepare");

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
        if name == "state_5.sqlite" {
            assert_eq!(
                fs::metadata(&link)
                    .expect("localized state metadata should read")
                    .len(),
                state_db_len
            );
        }
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

#[test]
fn caveman_developer_instructions_preserve_existing_config_and_are_idempotent() {
    let codex_home = temp_dir("caveman-developer-instructions");
    fs::create_dir_all(&codex_home).expect("codex home should exist");
    fs::write(
        codex_home.join("config.toml"),
        "developer_instructions = \"keep existing guidance\"\n",
    )
    .expect("base config should write");

    configure_caveman_launch_home(&codex_home).expect("caveman home should configure");
    configure_caveman_launch_home(&codex_home)
        .expect("caveman home configure should be idempotent");

    let config = fs::read_to_string(codex_home.join("config.toml")).expect("config should read");
    let parsed: toml::Value = toml::from_str(&config).expect("config should parse");
    let instructions = parsed["developer_instructions"]
        .as_str()
        .expect("developer instructions should be a string");
    assert!(instructions.starts_with("keep existing guidance"));
    assert_eq!(instructions.matches("CAVEMAN MODE ACTIVE").count(), 1);
    assert!(instructions.contains("PRODEX SUPER TOOLS ACTIVE WHEN AVAILABLE"));
    assert!(instructions.contains("Ponytail applies smallest-correct-implementation pressure"));
    assert!(instructions.contains("codebase-memory-mcp"));
    assert!(instructions.contains("Presidio is opt-in only"));
    assert!(parsed.get("hooks").is_none());
    assert!(
        !config.contains("zsh_path"),
        "Prodex overlay must not override Codex package-managed zsh discovery"
    );

    let _ = fs::remove_dir_all(codex_home);
}
