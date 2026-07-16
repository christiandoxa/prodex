use super::*;

#[test]
fn profile_export_envelope_rejects_oversized_bundle_before_reading() {
    let root = profile_export_private_temp_dir("oversized-read");
    let path = root.join("profiles.json");
    write_profile_export_bundle(&path, b"{}").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(PROFILE_EXPORT_BUNDLE_MAX_BYTES + 1)
        .unwrap();
    let error = read_profile_export_envelope::<serde_json::Value>(&path).unwrap_err();
    assert!(format!("{error:#}").contains("exceeds safe size limit"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn write_profile_export_bundle_rejects_oversized_payload() {
    let root = profile_export_private_temp_dir("oversized-write");
    let path = root.join("profiles.json");
    let content = vec![b' '; (PROFILE_EXPORT_BUNDLE_MAX_BYTES + 1) as usize];
    let error = write_profile_export_bundle(&path, &content).unwrap_err();
    assert!(format!("{error:#}").contains("exceeds safe size limit"));
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn profile_export_bundle_private_io_round_trips() {
    let root = profile_export_private_temp_dir("private-round-trip");
    let path = root.join("nested/profiles.json");
    let content =
        serialize_profile_export_payload(&serde_json::json!({ "profile": "main" }), None).unwrap();
    write_profile_export_bundle(&path, &content).unwrap();
    let (envelope, encrypted) = read_profile_export_envelope::<serde_json::Value>(&path).unwrap();
    assert!(!encrypted);
    assert!(
        matches!(envelope, ProfileExportEnvelope::Plain { payload, .. } if payload == serde_json::json!({ "profile": "main" }))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn profile_export_bundle_uses_no_follow_private_atomic_io() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink};
    let root = profile_export_private_temp_dir("private-no-follow");
    let outside = root.with_extension("outside");
    std::fs::write(&outside, "outside").unwrap();
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o600)).unwrap();
    let path = root.join("profiles.json");
    symlink(&outside, &path).unwrap();
    assert!(
        format!(
            "{:#}",
            read_profile_export_envelope::<serde_json::Value>(&path).unwrap_err()
        )
        .contains("failed to read")
    );
    write_profile_export_bundle(&path, b"inside").unwrap();
    assert!(!path.is_symlink());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "inside");
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "outside");
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    assert_eq!(metadata.uid(), std::fs::metadata(&root).unwrap().uid());
    let first_inode = metadata.ino();
    write_profile_export_bundle(&path, b"rotated").unwrap();
    assert_ne!(first_inode, std::fs::metadata(&path).unwrap().ino());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o770)).unwrap();
    let rejected = root.join("rejected.json");
    assert!(write_profile_export_bundle(&rejected, b"secret").is_err());
    assert!(!rejected.exists());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(outside);
}

#[cfg(unix)]
#[test]
fn profile_export_bundle_rejects_public_files_without_echoing_contents() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = profile_export_private_temp_dir("public-file-rejection");
    let path = root.join("profiles.json");
    let sentinel = "never-echo-this-export-content";
    std::fs::write(&path, format!(r#"{{"secret":"{sentinel}"}}"#)).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let rendered = format!(
        "{:#}",
        read_profile_export_envelope::<serde_json::Value>(&path).unwrap_err()
    );
    assert!(rendered.contains("failed to read"));
    assert!(!rendered.contains(sentinel));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn profile_export_bundle_rejects_reparse_point_reads() {
    use std::os::windows::fs::symlink_file;
    let root = profile_export_private_temp_dir("private-reparse");
    let target = root.join("target.json");
    let path = root.join("profiles.json");
    write_profile_export_bundle(&target, b"outside").unwrap();
    if let Err(error) = symlink_file(&target, &path) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            let _ = std::fs::remove_dir_all(root);
            return;
        }
        panic!("failed to create test reparse point: {error}");
    }
    assert!(
        format!(
            "{:#}",
            read_profile_export_envelope::<serde_json::Value>(&path).unwrap_err()
        )
        .contains("failed to read")
    );
    assert_eq!(std::fs::read_to_string(target).unwrap(), "outside");
    let _ = std::fs::remove_dir_all(root);
}

pub(super) fn profile_export_private_temp_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "prodex-profile-export-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    root
}
