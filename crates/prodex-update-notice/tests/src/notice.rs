use super::*;
use std::io::Cursor;

fn test_paths(name: &str) -> AppPaths {
    let root = env::temp_dir().join(format!(
        "prodex-update-notice-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("legacy-shared"),
        root,
    }
}

#[test]
fn update_check_cache_parse_error_redacts_cache_path() {
    let paths = test_paths("parse-redaction");
    fs::create_dir_all(&paths.root).expect("prodex root should be created");
    fs::write(update_check_cache_file_path(&paths), "{not-json").expect("cache should be written");

    let error = load_update_check_cache(&paths).unwrap_err().to_string();

    assert_eq!(error, "failed to parse update check cache");
    assert!(!error.contains(paths.root.to_string_lossy().as_ref()));

    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn install_channel_warnings_cover_legacy_managers() {
    assert_eq!(
        prodex_install_channel(
            None,
            Some(Path::new(
                r"C:\Users\test-user\AppData\Roaming\npm\node_modules\@christiandoxa\prodex-win32-x64\vendor\prodex.exe"
            ))
        ),
        ProdexInstallChannel::Npm
    );
    assert_eq!(
        prodex_install_channel(None, Some(Path::new("/home/test-user/.cargo/bin/prodex"))),
        ProdexInstallChannel::Cargo
    );
    assert_eq!(
        prodex_install_channel(None, Some(Path::new("/home/test-user/.local/bin/prodex"))),
        ProdexInstallChannel::Standalone
    );
    assert!(prodex_install_warning(ProdexInstallChannel::Npm).is_some());
    assert!(prodex_install_warning(ProdexInstallChannel::Cargo).is_some());
    assert!(prodex_install_warning(ProdexInstallChannel::Standalone).is_none());
}

#[test]
fn release_version_comparison_uses_mojo_total_order() {
    assert!(version_is_newer("v0.297.0", "0.296.0").unwrap());
    assert!(version_is_newer("1.10.0", "1.9.0").unwrap());
    assert!(version_is_newer("1.0.0", "1.0.0-rc.1").unwrap());
    assert!(version_is_newer("1.0.0-rc.10", "1.0.0-rc.2").unwrap());
    assert!(!version_is_newer("1.0.0-rc.1", "1.0.0").unwrap());
    assert!(version_is_newer("1.0.0+build.10", "1.0.0+build.2").unwrap());
    assert!(!version_is_newer("1.0.0+build-a", "1.0.0+build-a").unwrap());
    assert!(!version_is_newer("1.0.0+build-a", "1.0.0+build-b").unwrap());
    assert!(!version_is_newer("vv1.0.0", "1.0.0").unwrap());
    assert!(!version_is_newer("1.invalid.0", "1.0.0").unwrap());
    assert!(!version_is_newer("1.0.0", "invalid").unwrap());
    assert!(!version_is_newer("\u{2003}v1.0.0\u{3000}", "1.0.0").unwrap());
}

#[test]
fn update_decision_is_semver_aware_and_never_downgrades_automatically() {
    assert_eq!(
        prodex_update_decision("0.417.9", "0.418.0").unwrap(),
        ProdexUpdateDecision::UpdateAvailable("0.418.0".to_string())
    );
    assert_eq!(
        prodex_update_decision("0.418.0", "0.418.0").unwrap(),
        ProdexUpdateDecision::UpToDate
    );
    assert_eq!(
        prodex_update_decision("0.419.0-dev", "0.418.0").unwrap(),
        ProdexUpdateDecision::LocalNewer("0.418.0".to_string())
    );
    assert_eq!(
        prodex_update_decision("0.418.0-rc.1", "0.418.0").unwrap(),
        ProdexUpdateDecision::UpdateAvailable("0.418.0".to_string())
    );
    assert_eq!(
        prodex_update_decision("0.418.0+build-a", "0.418.0+build-b").unwrap(),
        ProdexUpdateDecision::UpToDate
    );
    assert_eq!(
        prodex_update_decision("0.418.0+build-b", "0.418.0+build-a").unwrap(),
        ProdexUpdateDecision::UpToDate
    );
    assert!(prodex_update_decision("0.418", "0.418.0").is_err());
    assert!(prodex_update_decision("vv0.418.0", "0.418.0").is_err());
}

#[test]
fn cached_update_refreshes_within_five_minutes() {
    assert_eq!(
        update_check_cache_ttl_seconds("0.3.1", "0.3.0").unwrap(),
        300
    );
    assert_eq!(
        update_check_cache_ttl_seconds("0.3.0", "0.3.0").unwrap(),
        300
    );
}

#[test]
fn cached_update_policy_preserves_source_age_and_clock_edges() {
    assert!(
        should_use_cached_update_version(
            ProdexReleaseSource::GitHub,
            "0.438.0",
            0,
            ProdexReleaseSource::GitHub,
            "0.437.5",
            299,
        )
        .unwrap()
    );
    assert!(
        !should_use_cached_update_version(
            ProdexReleaseSource::GitHub,
            "0.438.0",
            0,
            ProdexReleaseSource::GitHub,
            "0.437.5",
            300,
        )
        .unwrap()
    );
    assert!(
        !should_use_cached_update_version(
            ProdexReleaseSource::Npm,
            "0.438.0",
            0,
            ProdexReleaseSource::GitHub,
            "0.437.5",
            1,
        )
        .unwrap()
    );
    assert!(
        should_use_cached_update_version(
            ProdexReleaseSource::GitHub,
            "not-a-version",
            i64::MAX,
            ProdexReleaseSource::GitHub,
            "0.437.5",
            i64::MIN,
        )
        .unwrap()
    );
}

#[test]
fn prodex_version_status_uses_mojo_update_decision_for_cached_releases() {
    let newer_paths = test_paths("version-status-newer");
    save_update_check_cache(
        &newer_paths,
        &UpdateCheckCache {
            source: ProdexReleaseSource::GitHub,
            latest_version: "999.0.0".to_string(),
            checked_at: Local::now().timestamp(),
            ..UpdateCheckCache::default()
        },
    )
    .unwrap();
    assert_eq!(
        prodex_version_status(&newer_paths).unwrap(),
        ProdexVersionStatus::UpdateAvailable("999.0.0".to_string())
    );
    let _ = fs::remove_dir_all(newer_paths.root);

    let current_paths = test_paths("version-status-current");
    save_update_check_cache(
        &current_paths,
        &UpdateCheckCache {
            source: ProdexReleaseSource::GitHub,
            latest_version: current_prodex_version().to_string(),
            checked_at: Local::now().timestamp(),
            ..UpdateCheckCache::default()
        },
    )
    .unwrap();
    assert_eq!(
        prodex_version_status(&current_paths).unwrap(),
        ProdexVersionStatus::UpToDate
    );
    let _ = fs::remove_dir_all(current_paths.root);
}

#[test]
fn latest_release_version_comes_from_redirect_url() {
    let url = reqwest::Url::parse("https://github.com/christiandoxa/prodex/releases/tag/v0.300.0")
        .unwrap();

    assert_eq!(latest_release_version_from_url(&url).unwrap(), "0.300.0");
    assert!(
        latest_release_version_from_url(
            &reqwest::Url::parse("https://github.com/christiandoxa/prodex/releases/tag/vv0.300.0")
                .unwrap()
        )
        .is_err()
    );
    assert!(
        latest_release_version_from_url(
            &reqwest::Url::parse("https://github.com/christiandoxa/prodex/releases/latest")
                .unwrap()
        )
        .is_err()
    );
}

#[test]
fn latest_codex_release_version_comes_from_rust_tag() {
    let url =
        reqwest::Url::parse("https://github.com/openai/codex/releases/tag/rust-v0.144.6").unwrap();

    assert_eq!(
        latest_codex_release_version_from_url(&url).unwrap(),
        "0.144.6"
    );
}

#[test]
fn codex_info_version_uses_cached_github_release() {
    let paths = test_paths("codex-version-cache");
    let cache = UpdateCheckCache {
        source: ProdexReleaseSource::GitHub,
        latest_version: current_prodex_version().to_string(),
        checked_at: Local::now().timestamp(),
        codex_latest_version: Some("0.144.6".to_string()),
        codex_checked_at: Some(Local::now().timestamp()),
    };
    save_update_check_cache(&paths, &cache).unwrap();

    assert_eq!(
        format_info_codex_version(&paths, Some("0.144.5")).unwrap(),
        "0.144.5 (update available: 0.144.6)"
    );
    assert_eq!(
        format_info_codex_version(&paths, Some("0.144.6")).unwrap(),
        "0.144.6 (up to date)"
    );
    assert_eq!(
        format_info_codex_version(&paths, None).unwrap(),
        "not detected (latest release: 0.144.6)"
    );

    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn update_cache_write_is_private_and_bounded() {
    let paths = test_paths("private-cache");
    let cache = UpdateCheckCache {
        source: ProdexReleaseSource::GitHub,
        latest_version: "0.297.0".to_string(),
        checked_at: 1,
        ..UpdateCheckCache::default()
    };
    save_update_check_cache(&paths, &cache).unwrap();
    assert_eq!(
        load_update_check_cache(&paths)
            .unwrap()
            .unwrap()
            .latest_version,
        "0.297.0"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            fs::metadata(update_check_cache_file_path(&paths))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    fs::write(
        update_check_cache_file_path(&paths),
        vec![b'x'; UPDATE_CHECK_CACHE_MAX_BYTES as usize + 1],
    )
    .unwrap();
    assert!(load_update_check_cache(&paths).is_err());

    let _ = fs::remove_dir_all(paths.root);
}

#[test]
fn update_cache_reader_rejects_growth_past_limit() {
    let error = read_update_check_cache_content(Cursor::new(b"123456789"), 8).unwrap_err();

    assert!(error.to_string().contains("exceeds read limit"));
}

#[cfg(unix)]
#[test]
fn update_cache_reader_rejects_symlink() {
    use std::os::unix::fs::symlink;

    let paths = test_paths("symlink-cache");
    fs::create_dir_all(&paths.root).unwrap();
    let target = paths.root.join("outside.json");
    fs::write(
        &target,
        r#"{"source":"GitHub","latest_version":"0.297.0","checked_at":1}"#,
    )
    .unwrap();
    symlink(&target, update_check_cache_file_path(&paths)).unwrap();

    let error = load_update_check_cache(&paths).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("failed to open update check cache")
    );

    let _ = fs::remove_dir_all(paths.root);
}
