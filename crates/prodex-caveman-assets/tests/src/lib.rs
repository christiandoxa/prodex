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
fn configure_rtk_codex_home_writes_awareness_and_agents_reference() {
    let dir = temp_dir("rtk");
    configure_rtk_codex_home(&dir).expect("rtk codex home should configure");

    let rtk_md = fs::read_to_string(dir.join("RTK.md")).expect("RTK.md should exist");
    assert!(rtk_md.contains("RTK - Rust Token Killer"));
    assert!(rtk_md.contains("rtk gain"));

    let agents = fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md should exist");
    assert_eq!(agents, format!("@{}\n", dir.join("RTK.md").display()));

    configure_rtk_codex_home(&dir).expect("rtk codex home should be idempotent");
    let agents = fs::read_to_string(dir.join("AGENTS.md")).expect("AGENTS.md should exist");
    assert_eq!(agents.matches("@").count(), 1);

    let _ = fs::remove_dir_all(dir);
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

    let _ = fs::remove_dir_all(base);
    let _ = fs::remove_dir_all(managed_root);
}
