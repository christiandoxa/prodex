use super::*;

#[cfg(unix)]
#[test]
fn external_file_backend_allows_readable_cli_permissions_but_not_writes() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = temp_dir("external-source-permissions");
    let path = root.join(".credentials.json");
    fs::write(&path, "external-value").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let backend = FileSecretBackend::new();

    assert_eq!(
        backend
            .read_external_text_bounded(&path, 64)
            .unwrap()
            .as_deref(),
        Some("external-value")
    );
    assert!(
        SecretManager::new(backend)
            .read_text(&SecretLocation::file(&path))
            .is_err()
    );

    fs::set_permissions(&path, fs::Permissions::from_mode(0o664)).unwrap();
    let error = backend.read_external_text_bounded(&path, 64).unwrap_err();
    assert!(error.is_unsafe_file());

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn external_file_backend_rejects_fifo_without_blocking() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;
    use std::process::Command;
    use std::time::Instant;

    if let Some(path) = std::env::var_os("PRODEX_SECRET_STORE_FIFO_TEST_PATH") {
        let error = FileSecretBackend::new()
            .read_external_text_bounded(&PathBuf::from(path), 64)
            .unwrap_err();
        assert!(error.is_unsafe_file());
        return;
    }

    let root = temp_dir("external-source-fifo");
    let path = root.join(".credentials.json");
    let path_bytes = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: path_bytes is a valid NUL-terminated path and the mode is
    // intentionally private to the current test user.
    assert_eq!(unsafe { libc::mkfifo(path_bytes.as_ptr(), 0o600) }, 0);

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "external::external_file_backend_rejects_fifo_without_blocking",
            "--nocapture",
        ])
        .env("PRODEX_SECRET_STORE_FIFO_TEST_PATH", &path)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("external FIFO read did not finish before its deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn external_file_backend_rejects_symlink_parent_traversal_and_oversized_files() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let root = temp_dir("external-source-boundary");
    let target = root.join("target.json");
    let symlink_path = root.join(".credentials.json");
    fs::write(&target, "external-value").unwrap();
    symlink(&target, &symlink_path).unwrap();
    let backend = FileSecretBackend::new();
    assert!(
        backend
            .read_external_text_bounded(&symlink_path, 64)
            .unwrap_err()
            .is_unsafe_file()
    );

    let outside = root.with_extension("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join(".credentials.json"), "outside-value").unwrap();
    let linked_parent = root.join("linked");
    symlink(&outside, &linked_parent).unwrap();
    let linked_path = linked_parent.join(".credentials.json");
    assert!(
        backend
            .read_external_text_bounded(&linked_path, 64)
            .unwrap_err()
            .is_unsafe_file()
    );

    let traversal = root.join("../external-source.json");
    assert!(matches!(
        backend.read_external_text_bounded(&traversal, 64),
        Err(SecretError::InvalidLocation { .. })
    ));

    let oversized = root.join("oversized.json");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(64 * 1024 + 1)
        .unwrap();
    fs::set_permissions(&oversized, fs::Permissions::from_mode(0o600)).unwrap();
    let error = backend
        .read_external_text_bounded(&oversized, 64 * 1024)
        .unwrap_err();
    assert_eq!(
        error.invalid_location_kind(),
        Some(crate::SecretInvalidLocationKind::SizeLimitExceeded)
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[cfg(windows)]
#[test]
fn external_file_backend_rejects_reparse_point_sources() {
    use std::os::windows::fs::symlink_file;

    let root = temp_dir("external-source-reparse");
    let target = root.join("target.json");
    let path = root.join(".credentials.json");
    fs::write(&target, "outside").unwrap();
    if let Err(error) = symlink_file(&target, &path) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            let _ = fs::remove_dir_all(root);
            return;
        }
        panic!("failed to create test reparse point: {error}");
    }

    let error = FileSecretBackend::new()
        .read_external_text_bounded(&path, 64)
        .unwrap_err();
    assert!(error.is_unsafe_file());
    assert_eq!(fs::read_to_string(target).unwrap(), "outside");
    let _ = fs::remove_dir_all(root);
}
