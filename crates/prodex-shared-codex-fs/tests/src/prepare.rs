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
