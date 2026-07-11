use super::*;

#[test]
fn app_paths_discover_resolves_relative_shared_codex_home_inside_prodex_root() {
    let temp_dir = TestDir::isolated();
    let prodex_home = temp_dir.path.join("prodex");
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let _shared = TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", ".codex-local");

    let paths = AppPaths::discover().expect("paths should resolve");
    assert_eq!(paths.shared_codex_root, paths.root.join(".codex-local"));
}

#[test]
fn parses_email_from_chatgpt_id_token() {
    let id_token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL3Byb2ZpbGUiOnsiZW1haWwiOiJ1c2VyQGV4YW1wbGUuY29tIn19.c2ln";

    assert_eq!(
        parse_email_from_id_token(id_token).expect("id token should parse"),
        Some("user@example.com".to_string())
    );
}

#[test]
fn app_paths_discover_uses_native_codex_home_for_default_shared_codex_home() {
    let temp_dir = TestDir::isolated();
    let home_dir = temp_dir.path.join("home");
    let prodex_home = temp_dir.path.join("prodex");
    let home_dir_string = home_dir.to_string_lossy().to_string();
    let prodex_home_string = prodex_home.to_string_lossy().to_string();
    let _home = TestEnvVarGuard::set("HOME", &home_dir_string);
    let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", &prodex_home_string);
    let _shared = TestEnvVarGuard::unset("PRODEX_SHARED_CODEX_HOME");

    let paths = AppPaths::discover().expect("paths should resolve");
    assert_eq!(paths.root, prodex_home);
    assert_eq!(paths.shared_codex_root, home_dir.join(".codex"));
    assert_eq!(paths.legacy_shared_codex_root, paths.root.join("shared"));
}

#[test]
fn select_default_codex_home_prefers_legacy_home_until_prodex_shared_home_exists() {
    let temp_dir = TestDir::isolated();
    let shared_codex_home = temp_dir.path.join("prodex/.codex");
    let legacy_codex_home = temp_dir.path.join("home/.codex");
    fs::create_dir_all(&legacy_codex_home).expect("legacy codex home should exist");

    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, false),
        legacy_codex_home
    );

    fs::create_dir_all(&shared_codex_home).expect("prodex shared codex home should exist");
    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, false),
        shared_codex_home
    );

    fs::remove_dir_all(&shared_codex_home).expect("shared codex home should be removed");
    assert_eq!(
        select_default_codex_home(&shared_codex_home, &legacy_codex_home, true),
        shared_codex_home
    );
}
