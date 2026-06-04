use super::*;
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
