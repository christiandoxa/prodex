use super::*;
use filetime::FileTime;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

struct PrepareTestDir {
    path: PathBuf,
}

impl PrepareTestDir {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "prodex-shared-prepare-{name}-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("test dir should be created");
        Self { path }
    }

    fn app_paths(&self) -> AppPaths {
        AppPaths {
            root: self.path.join("prodex"),
            state_file: self.path.join("prodex/state.json"),
            managed_profiles_root: self.path.join("prodex/profiles"),
            shared_codex_root: self.path.join("shared-codex"),
            legacy_shared_codex_root: self.path.join("legacy-shared-codex"),
        }
    }
}

impl Drop for PrepareTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn shared_codex_manifest_includes_environments_toml_as_file() {
    assert!(shared_codex_manifest_entries().contains(&SharedCodexEntry::file("environments.toml")));
}

#[test]
fn shared_codex_manifest_includes_managed_config_toml_as_file() {
    assert!(
        shared_codex_manifest_entries().contains(&SharedCodexEntry::file("managed_config.toml"))
    );
}

#[test]
fn shared_codex_manifest_includes_mcp_oauth_fallback_credentials_as_file() {
    assert!(shared_codex_manifest_entries().contains(&SharedCodexEntry::file(".credentials.json")));
}

#[test]
fn shared_codex_manifest_includes_plugin_marketplace_state() {
    let entries = shared_codex_manifest_entries();
    for name in [
        "plugins",
        ".tmp/plugins",
        ".tmp/marketplaces",
        ".tmp/known_marketplaces.json",
        ".tmp/app-server-remote-plugin-sync-v1",
    ] {
        let expected = if name.ends_with(".json") || name.ends_with("-v1") {
            SharedCodexEntry::file(name)
        } else {
            SharedCodexEntry::directory(name)
        };
        assert!(
            entries.contains(&expected),
            "shared Codex manifest should include {name}"
        );
    }
}

#[test]
fn shared_codex_manifest_keeps_cloud_config_bundle_cache_profile_local() {
    assert!(
        !shared_codex_manifest_entries()
            .contains(&SharedCodexEntry::file("cloud-config-bundle-cache.json"))
    );
}

#[test]
fn profile_v2_config_names_match_plain_ascii_profile_names() {
    assert!(is_shared_codex_profile_v2_config_name(
        "local_1-prod.config.toml"
    ));
    assert!(!is_shared_codex_profile_v2_config_name(".config.toml"));
    assert!(!is_shared_codex_profile_v2_config_name(
        "team.prod.config.toml"
    ));
    assert!(!is_shared_codex_profile_v2_config_name(
        "../prod.config.toml"
    ));
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_restores_session_mtime_from_last_timestamp() {
    let temp_dir = PrepareTestDir::new("session-mtime");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let session_file = paths
        .shared_codex_root
        .join("sessions/2026/06/19/rollout-2026-06-19T08-00-00-session.jsonl");
    let stale_mtime = FileTime::from_unix_time(1_800_000_000, 0);
    let expected_mtime = FileTime::from_unix_time(1_766_132_168, 0);

    fs::create_dir_all(session_file.parent().expect("session parent"))
        .expect("session parent should be created");
    fs::write(
        &session_file,
        concat!(
            r#"{"timestamp":"2025-12-19T08:00:00Z","type":"session_meta"}"#,
            "\n",
            r#"{"timestamp":"2025-12-19T08:16:08Z","type":"response_item"}"#,
            "\n"
        ),
    )
    .expect("session file should write");
    filetime::set_file_mtime(&session_file, stale_mtime).expect("session mtime should update");

    prepare_managed_codex_home(&paths, &codex_home).expect("managed codex home should be prepared");

    let restored_mtime = FileTime::from_last_modification_time(
        &fs::metadata(&session_file).expect("session metadata should read"),
    );
    assert_eq!(restored_mtime, expected_mtime);
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_only_reprocesses_changed_sessions() {
    let temp_dir = PrepareTestDir::new("incremental-session-maintenance");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let session_file = paths
        .shared_codex_root
        .join("sessions/2026/06/21/rollout-session.jsonl");
    fs::create_dir_all(session_file.parent().expect("session parent")).expect("session parent");
    fs::write(
        &session_file,
        "{\"timestamp\":\"2026-06-21T08:00:00Z\",\"type\":\"session_meta\"}\n",
    )
    .expect("session should write");

    prepare_managed_codex_home(&paths, &codex_home).expect("first prepare should succeed");
    let cache_path = paths.root.join(SESSION_MAINTENANCE_CACHE_FILE);
    let first_cache = fs::read(&cache_path).expect("maintenance cache should exist");

    prepare_managed_codex_home(&paths, &codex_home).expect("unchanged prepare should succeed");
    assert_eq!(
        fs::read(&cache_path).expect("maintenance cache should remain readable"),
        first_cache
    );

    fs::OpenOptions::new()
        .append(true)
        .open(&session_file)
        .expect("session should open")
        .write_all(b"{\"timestamp\":\"2026-06-21T08:05:00Z\",\"type\":\"response_item\"}\n")
        .expect("session should append");
    prepare_managed_codex_home(&paths, &codex_home).expect("changed prepare should succeed");

    let restored_mtime = FileTime::from_last_modification_time(
        &fs::metadata(&session_file).expect("session metadata should read"),
    );
    assert_eq!(restored_mtime, FileTime::from_unix_time(1_782_029_100, 0));
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_retries_attachment_that_appears_later() {
    let temp_dir = PrepareTestDir::new("late-session-attachment");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let session_file = paths
        .shared_codex_root
        .join("sessions/2026/06/21/rollout-session.jsonl");
    let source = temp_dir.path.join("codex-clipboard-late.png");
    fs::create_dir_all(session_file.parent().expect("session parent")).expect("session parent");
    fs::write(
        &session_file,
        format!(
            r#"{{"timestamp":"2026-06-21T08:00:00Z","text":"<image path=\"{}\">"}}"#,
            source.display()
        ),
    )
    .expect("session should write");

    prepare_managed_codex_home(&paths, &codex_home).expect("first prepare should succeed");
    let stable = paths
        .shared_codex_root
        .join("image_attachments/codex-clipboard-late.png");
    assert!(!stable.exists());

    fs::write(&source, b"late image").expect("late source should write");
    prepare_managed_codex_home(&paths, &codex_home).expect("second prepare should retry");

    assert_eq!(
        fs::read(&stable).expect("stable image should exist"),
        b"late image"
    );
    let rewritten = fs::read_to_string(&session_file).expect("session should read");
    assert!(rewritten.contains(&stable.display().to_string()));
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_rescans_v2_cache_for_attachment_image_paths() {
    let temp_dir = PrepareTestDir::new("v2-cache-attachment-image-rescan");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let session_file = paths
        .shared_codex_root
        .join("sessions/2026/06/25/rollout-session.jsonl");
    let source = temp_dir
        .path
        .join("overlay/attachments/31e02015-1740-4a23-85fe-51cf33a476e6/image-1.png");
    fs::create_dir_all(session_file.parent().expect("session parent")).expect("session parent");
    fs::create_dir_all(source.parent().expect("source parent")).expect("source parent");
    fs::write(&source, b"goal resume image").expect("source image should write");
    fs::write(
        &session_file,
        format!(
            r#"{{"timestamp":"2026-06-25T08:00:00Z","type":"event_msg","payload":{{"goal":{{"objective":"image file: {}"}}}}}}"#,
            source.display()
        ),
    )
    .expect("session should write");

    let cache_key = session_file
        .strip_prefix(&paths.shared_codex_root)
        .expect("session should be under shared root")
        .to_string_lossy()
        .into_owned();
    let stale_v2_cache = SessionMaintenanceCache {
        version: 2,
        files: BTreeMap::from([(cache_key, session_file_fingerprint(&session_file).unwrap())]),
    };
    fs::create_dir_all(&paths.root).expect("prodex root should exist");
    fs::write(
        paths.root.join(SESSION_MAINTENANCE_CACHE_FILE),
        serde_json::to_vec(&stale_v2_cache).expect("cache should serialize"),
    )
    .expect("stale cache should write");

    prepare_managed_codex_home(&paths, &codex_home).expect("prepare should rescan old cache");

    let stable = paths
        .shared_codex_root
        .join("attachments/31e02015-1740-4a23-85fe-51cf33a476e6/image-1.png");
    assert_eq!(
        fs::read(&stable).expect("stable image attachment should exist"),
        b"goal resume image"
    );
    let rewritten = fs::read_to_string(&session_file).expect("session should read");
    assert!(rewritten.contains(&stable.display().to_string()));
    assert!(!rewritten.contains(&source.display().to_string()));
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_ignores_cache_write_failure() {
    let temp_dir = PrepareTestDir::new("cache-write-failure");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let session_file = paths.shared_codex_root.join("sessions/rollout.jsonl");
    fs::create_dir_all(session_file.parent().expect("session parent")).expect("session parent");
    fs::write(
        &session_file,
        "{\"timestamp\":\"2026-06-21T08:00:00Z\",\"type\":\"session_meta\"}\n",
    )
    .expect("session should write");
    fs::create_dir_all(paths.root.join(SESSION_MAINTENANCE_CACHE_FILE))
        .expect("directory should block cache file writes");

    prepare_managed_codex_home(&paths, &codex_home)
        .expect("cache failure must not block managed home preparation");
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_detects_same_size_same_mtime_replacement() {
    let temp_dir = PrepareTestDir::new("same-metadata-session-replacement");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let session_file = paths.shared_codex_root.join("sessions/rollout.jsonl");
    fs::create_dir_all(session_file.parent().expect("session parent")).expect("session parent");
    let first = "{\"timestamp\":\"2026-06-21T08:00:00Z\",\"type\":\"session_meta\"}\n";
    let second = "{\"timestamp\":\"2026-06-21T09:00:00Z\",\"type\":\"session_meta\"}\n";
    assert_eq!(first.len(), second.len());
    fs::write(&session_file, first).expect("first session should write");
    prepare_managed_codex_home(&paths, &codex_home).expect("first prepare should succeed");
    let cached_mtime = FileTime::from_last_modification_time(
        &fs::metadata(&session_file).expect("first metadata should read"),
    );

    fs::write(&session_file, second).expect("replacement session should write");
    filetime::set_file_mtime(&session_file, cached_mtime).expect("mtime should match cache");
    prepare_managed_codex_home(&paths, &codex_home).expect("replacement should be detected");

    let restored_mtime = FileTime::from_last_modification_time(
        &fs::metadata(&session_file).expect("replacement metadata should read"),
    );
    assert_eq!(restored_mtime, FileTime::from_unix_time(1_782_032_400, 0));
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_migrates_plugin_cache_and_marketplace_state_to_shared_root() {
    let temp_dir = PrepareTestDir::new("plugin-cache-marketplaces");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let plugin_manifest = codex_home
        .join("plugins/cache/openai-curated/sample-plugin/1.2.3/.codex-plugin/plugin.json");
    let marketplace_manifest = codex_home.join(".tmp/marketplaces/openai-curated/marketplace.json");
    let known_marketplaces = codex_home.join(".tmp/known_marketplaces.json");
    let remote_sync_marker = codex_home.join(".tmp/app-server-remote-plugin-sync-v1");

    fs::create_dir_all(plugin_manifest.parent().unwrap()).expect("plugin cache dir");
    fs::create_dir_all(marketplace_manifest.parent().unwrap()).expect("marketplace dir");
    fs::write(&plugin_manifest, r#"{"name":"sample-plugin"}"#).expect("plugin manifest");
    fs::write(&marketplace_manifest, r#"{"plugins":[]}"#).expect("marketplace manifest");
    fs::write(&known_marketplaces, r#"{"marketplaces":[]}"#).expect("known marketplaces");
    fs::write(&remote_sync_marker, "synced").expect("remote plugin sync marker");

    prepare_managed_codex_home(&paths, &codex_home).expect("managed codex home should be prepared");

    assert_eq!(
        fs::read_to_string(
            paths
                .shared_codex_root
                .join("plugins/cache/openai-curated/sample-plugin/1.2.3/.codex-plugin/plugin.json")
        )
        .expect("shared plugin manifest should read"),
        r#"{"name":"sample-plugin"}"#
    );
    assert_eq!(
        fs::read_to_string(
            paths
                .shared_codex_root
                .join(".tmp/marketplaces/openai-curated/marketplace.json")
        )
        .expect("shared marketplace manifest should read"),
        r#"{"plugins":[]}"#
    );
    assert_eq!(
        fs::read_to_string(
            codex_home
                .join("plugins/cache/openai-curated/sample-plugin/1.2.3/.codex-plugin/plugin.json")
        )
        .expect("profile plugin symlink should remain readable"),
        r#"{"name":"sample-plugin"}"#
    );
    assert_eq!(
        fs::read_link(codex_home.join("plugins")).expect("plugins should be shared by symlink"),
        paths.shared_codex_root.join("plugins")
    );
    assert_eq!(
        fs::read_to_string(paths.shared_codex_root.join(".tmp/known_marketplaces.json"))
            .expect("known marketplaces should move to shared root"),
        r#"{"marketplaces":[]}"#
    );
    assert_eq!(
        fs::read_to_string(
            paths
                .shared_codex_root
                .join(".tmp/app-server-remote-plugin-sync-v1")
        )
        .expect("remote plugin sync marker should move to shared root"),
        "synced"
    );
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_migrates_profile_v2_config_to_shared_root() {
    let temp_dir = PrepareTestDir::new("profile-v2-config-file");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let local_path = codex_home.join("bedrock.config.toml");
    let shared_path = paths.shared_codex_root.join("bedrock.config.toml");

    fs::create_dir_all(&codex_home).expect("codex home should be created");
    fs::write(&local_path, "model_provider = \"amazon-bedrock\"\n")
        .expect("profile v2 config should write");

    prepare_managed_codex_home(&paths, &codex_home).expect("managed codex home should be prepared");

    assert_eq!(
        fs::read_to_string(&shared_path).expect("shared profile v2 config should be readable"),
        "model_provider = \"amazon-bedrock\"\n"
    );
    assert_eq!(
        fs::read_link(&local_path).expect("local profile v2 config should be a symlink"),
        shared_path
    );
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_links_existing_shared_profile_v2_config() {
    let temp_dir = PrepareTestDir::new("existing-profile-v2-config-file");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let local_path = codex_home.join("bedrock.config.toml");
    let shared_path = paths.shared_codex_root.join("bedrock.config.toml");

    fs::create_dir_all(&paths.shared_codex_root).expect("shared root should be created");
    fs::write(&shared_path, "model_provider = \"amazon-bedrock\"\n")
        .expect("shared profile v2 config should write");

    prepare_managed_codex_home(&paths, &codex_home).expect("managed codex home should be prepared");

    assert_eq!(
        fs::read_link(&local_path).expect("local profile v2 config should be a symlink"),
        shared_path
    );
    assert_eq!(
        fs::read_to_string(&local_path).expect("local profile v2 config link should be readable"),
        "model_provider = \"amazon-bedrock\"\n"
    );
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_migrates_goals_sqlite_files_to_shared_root() {
    let temp_dir = PrepareTestDir::new("goals-sqlite-files");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let goals_files = [
        ("goals_1.sqlite", "main db"),
        ("goals_1.sqlite-shm", "shared memory"),
        ("goals_1.sqlite-wal", "write ahead log"),
    ];

    fs::create_dir_all(&codex_home).expect("codex home should be created");
    for (file_name, contents) in goals_files {
        fs::write(codex_home.join(file_name), contents).expect("goals sqlite file should write");
    }

    prepare_managed_codex_home(&paths, &codex_home).expect("managed codex home should be prepared");

    for (file_name, contents) in goals_files {
        let local_path = codex_home.join(file_name);
        let shared_path = paths.shared_codex_root.join(file_name);
        assert_eq!(
            fs::read_to_string(&shared_path).expect("shared goals sqlite file should be readable"),
            contents
        );
        assert_eq!(
            fs::read_link(&local_path).expect("local goals sqlite file should be a symlink"),
            shared_path
        );
    }
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_migrates_memories_sqlite_files_to_shared_root() {
    let temp_dir = PrepareTestDir::new("memories-sqlite-files");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let memories_files = [
        ("memories_1.sqlite", "main db"),
        ("memories_1.sqlite-shm", "shared memory"),
        ("memories_1.sqlite-wal", "write ahead log"),
    ];

    fs::create_dir_all(&codex_home).expect("codex home should be created");
    for (file_name, contents) in memories_files {
        fs::write(codex_home.join(file_name), contents).expect("memories sqlite file should write");
    }

    prepare_managed_codex_home(&paths, &codex_home).expect("managed home should prepare");

    for (file_name, contents) in memories_files {
        let local_path = codex_home.join(file_name);
        let shared_path = paths.shared_codex_root.join(file_name);
        assert_eq!(
            fs::read_to_string(&shared_path).expect("shared memories sqlite file should read"),
            contents
        );
        assert_eq!(
            fs::read_link(&local_path).expect("local memories sqlite path should be a symlink"),
            shared_path
        );
    }
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_migrates_environments_toml_to_shared_root() {
    let temp_dir = PrepareTestDir::new("environments-file");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let local_path = codex_home.join("environments.toml");
    let shared_path = paths.shared_codex_root.join("environments.toml");

    fs::create_dir_all(&codex_home).expect("codex home should be created");
    fs::write(&local_path, "[env.local]\nvalue = \"profile\"\n")
        .expect("environment file should write");

    prepare_managed_codex_home(&paths, &codex_home).expect("managed codex home should be prepared");

    assert_eq!(
        fs::read_to_string(&shared_path).expect("shared environments should be readable"),
        "[env.local]\nvalue = \"profile\"\n"
    );
    assert_eq!(
        fs::read_link(&local_path).expect("local environments should be a symlink"),
        shared_path
    );
    assert_eq!(
        fs::read_to_string(&local_path).expect("local environments link should be readable"),
        "[env.local]\nvalue = \"profile\"\n"
    );
}

#[cfg(unix)]
#[test]
fn prepare_managed_codex_home_copies_environments_toml_symlink_target_to_shared_root() {
    let temp_dir = PrepareTestDir::new("environments-symlink");
    let paths = temp_dir.app_paths();
    let codex_home = temp_dir.path.join("profile-codex-home");
    let local_path = codex_home.join("environments.toml");
    let source_path = temp_dir.path.join("source-environments.toml");
    let shared_path = paths.shared_codex_root.join("environments.toml");

    fs::create_dir_all(&codex_home).expect("codex home should be created");
    fs::write(&source_path, "[env.external]\nvalue = \"linked\"\n")
        .expect("source environment file should write");
    std::os::unix::fs::symlink(&source_path, &local_path)
        .expect("local environments symlink should be created");

    prepare_managed_codex_home(&paths, &codex_home).expect("managed codex home should be prepared");

    assert_eq!(
        fs::read_to_string(&shared_path).expect("shared environments should be readable"),
        "[env.external]\nvalue = \"linked\"\n"
    );
    assert_eq!(
        fs::read_link(&local_path).expect("local environments should be relinked"),
        shared_path
    );
}
