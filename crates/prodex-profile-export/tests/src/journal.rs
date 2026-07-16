use super::*;

#[test]
fn import_auth_update_journal_constructor_sets_current_version() {
    let journal = ImportedExistingProfileAuthUpdateJournal::new(
        "main".to_string(),
        "/tmp/main".to_string(),
        Some("old@example.com".to_string()),
        Some("{}".to_string()),
        "2026-05-02T00:00:00+00:00".to_string(),
    );
    assert_eq!(journal.version, IMPORT_AUTH_UPDATE_JOURNAL_VERSION);
    validate_import_auth_update_journal_version(journal.version).unwrap();
    assert!(validate_import_auth_update_journal_version(journal.version + 1).is_err());
}

#[test]
fn import_auth_update_journal_reader_parses_current_version() {
    let root = profile_export_private_temp_dir("journal-read");
    let path = root.join("main.json");
    let journal = ImportedExistingProfileAuthUpdateJournal::new(
        "main".to_string(),
        "/tmp/main".to_string(),
        Some("old@example.com".to_string()),
        Some("{}".to_string()),
        "2026-05-02T00:00:00+00:00".to_string(),
    );
    write_profile_import_auth_update_journal(&path, &journal).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    assert_eq!(
        read_profile_import_auth_update_journal(&path).unwrap(),
        journal
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn import_auth_update_journal_reader_rejects_oversized_file() {
    let root = profile_export_private_temp_dir("journal-oversized");
    let path = root.join("main.json");
    secret_store::write_private_file_atomic(&path, b"").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    let error = read_profile_import_auth_update_journal(&path).unwrap_err();
    assert!(format!("{error:#}").contains("exceeds safe size limit"));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn import_auth_update_journal_reader_rejects_symlink() {
    let root = profile_export_private_temp_dir("journal-read-symlink");
    let target = root.join("target.json");
    let path = root.join("main.json");
    std::fs::write(&target, "{}").unwrap();
    std::os::unix::fs::symlink(&target, &path).unwrap();
    assert!(
        read_profile_import_auth_update_journal(&path)
            .unwrap_err()
            .to_string()
            .contains("is not a regular file")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn import_auth_update_journal_paths_ignore_symlinks() {
    let root = profile_export_private_temp_dir("journal-symlink");
    let journal_root = ensure_profile_import_auth_update_journal_root(&root).unwrap();
    let real_journal = journal_root.join("main-real.json");
    let linked_journal = root.join("linked.json");
    std::fs::write(&real_journal, "{}").unwrap();
    std::fs::write(&linked_journal, "{}").unwrap();
    std::os::unix::fs::symlink(&linked_journal, journal_root.join("main-linked.json")).unwrap();
    assert_eq!(
        profile_import_auth_update_journal_paths(&root).unwrap(),
        vec![real_journal]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn import_auth_update_journal_root_rejects_symlink() {
    let root = profile_export_private_temp_dir("journal-root-symlink");
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, profile_import_auth_update_journal_root(&root)).unwrap();
    let list_error = profile_import_auth_update_journal_paths(&root).unwrap_err();
    assert!(list_error.to_string().contains("is a symlink"));
    let ensure_error = ensure_profile_import_auth_update_journal_root(&root).unwrap_err();
    assert!(ensure_error.to_string().contains("is a symlink"));
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
    let _ = std::fs::remove_dir_all(root);
}
