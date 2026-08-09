use super::*;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir()
        .canonicalize()
        .expect("temporary directory should resolve")
        .join(format!(
            "prodex-optional-tools-{name}-{}-{stamp}",
            std::process::id()
        ))
}

#[test]
fn optional_tool_ids_parse_to_typed_values() {
    assert_eq!("caveman".parse(), Ok(OptionalToolId::Caveman));
    assert_eq!("cbm".parse(), Ok(OptionalToolId::CodebaseMemoryMcp));
    assert!("unknown".parse::<OptionalToolId>().is_err());
}

#[test]
fn super_defaults_are_typed_and_presidio_is_opt_in() {
    let tools = OptionalToolSet::super_defaults();
    assert!(tools.contains(OptionalToolId::Caveman));
    assert!(tools.contains(OptionalToolId::Rtk));
    assert!(tools.contains(OptionalToolId::PlaywrightMcp));
    assert!(!tools.contains(OptionalToolId::Presidio));
}

#[test]
fn configure_rtk_codex_home_is_idempotent() {
    let home = temp_dir("rtk");
    configure_rtk_codex_home(&home).unwrap();
    let first_agents = fs::read(home.join("AGENTS.md")).unwrap();
    let first_awareness = fs::read(home.join("RTK.md")).unwrap();
    configure_rtk_codex_home(&home).unwrap();
    assert_eq!(fs::read(home.join("AGENTS.md")).unwrap(), first_agents);
    assert_eq!(fs::read(home.join("RTK.md")).unwrap(), first_awareness);
    let _ = fs::remove_dir_all(home);
}

#[test]
fn configure_overlay_removes_only_legacy_prodex_caveman_entries() {
    let home = temp_dir("legacy-cleanup");
    fs::create_dir_all(home.join(".tmp/marketplaces/prodex-caveman")).unwrap();
    fs::create_dir_all(home.join("plugins/cache/prodex-caveman/caveman")).unwrap();
    fs::create_dir_all(home.join("plugins/cache/keep")).unwrap();
    fs::write(
        home.join("config.toml"),
        r#"model = "gpt-5"

[marketplaces.prodex-caveman]
source_type = "git"

[marketplaces.keep]
source_type = "directory"

[plugins."caveman@prodex-caveman"]
enabled = true

[plugins.keep]
enabled = true
"#,
    )
    .unwrap();

    configure_prodex_overlay_home(&home).unwrap();
    let first = fs::read(home.join("config.toml")).unwrap();
    configure_prodex_overlay_home(&home).unwrap();

    assert_eq!(fs::read(home.join("config.toml")).unwrap(), first);
    let config = String::from_utf8(first).unwrap();
    assert!(!config.contains("prodex-caveman"));
    assert!(config.contains("marketplaces.keep"));
    assert!(config.contains("plugins.keep"));
    assert!(!home.join(".tmp/marketplaces/prodex-caveman").exists());
    assert!(!home.join("plugins/cache/prodex-caveman").exists());
    assert!(home.join("plugins/cache/keep").is_dir());
    let _ = fs::remove_dir_all(home);
}

#[test]
fn runtime_overlay_preserves_original_config_and_has_no_caveman_payload() {
    let managed = temp_dir("runtime-managed");
    let base = temp_dir("runtime-base");
    fs::create_dir_all(&base).unwrap();
    fs::write(base.join("config.toml"), "model = 'gpt-5'\n").unwrap();

    let overlay = prepare_runtime_overlay_home(&managed, &base).unwrap();

    assert_eq!(
        fs::read_to_string(overlay.join("config.toml")).unwrap(),
        "model = 'gpt-5'\n"
    );
    assert!(!overlay.join("skills/caveman").exists());
    assert!(!overlay.join("plugins/cache/prodex-caveman").exists());
    let _ = fs::remove_dir_all(managed);
    let _ = fs::remove_dir_all(base);
}

#[test]
fn overlay_drops_inherited_codex_app_cache_only() {
    let managed = temp_dir("cache-managed");
    let base = temp_dir("cache-base");
    for relative in [
        "cache/codex_apps_server_info",
        "cache/codex_apps_tools",
        "cache/codex_app_directory",
        "cache/keep",
    ] {
        fs::create_dir_all(base.join(relative)).unwrap();
    }
    fs::write(base.join("cache/keep/value"), "keep").unwrap();

    let overlay = prepare_prodex_overlay_home(&managed, &base).unwrap();

    assert!(!overlay.join("cache/codex_apps_server_info").exists());
    assert!(!overlay.join("cache/codex_apps_tools").exists());
    assert!(!overlay.join("cache/codex_app_directory").exists());
    assert!(overlay.join("cache/keep/value").is_file());
    let _ = fs::remove_dir_all(managed);
    let _ = fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn overlay_rejects_symlink_managed_root() {
    let root = temp_dir("symlink-root");
    let managed = root.join("managed");
    let outside = root.join("outside");
    let base = root.join("base");
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(&base).unwrap();
    std::os::unix::fs::symlink(&outside, &managed).unwrap();

    let error = prepare_prodex_overlay_home(&managed, &base).unwrap_err();

    assert!(error.to_string().contains("must not be a symbolic link"));
    assert!(fs::read_dir(&outside).unwrap().next().is_none());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn overlay_secures_existing_managed_root() {
    use std::os::unix::fs::PermissionsExt as _;

    let managed = temp_dir("secure-root-managed");
    let base = temp_dir("secure-root-base");
    fs::create_dir_all(&managed).unwrap();
    fs::create_dir_all(&base).unwrap();
    fs::set_permissions(&managed, fs::Permissions::from_mode(0o775)).unwrap();

    let overlay = prepare_prodex_overlay_home(&managed, &base).unwrap();

    assert_eq!(
        fs::metadata(&managed).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let _ = fs::remove_dir_all(managed);
    let _ = fs::remove_dir_all(base);
    assert!(!overlay.exists());
}
