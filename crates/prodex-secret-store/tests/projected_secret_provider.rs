use prodex_domain::{
    SecretProvider, SecretProviderKind, SecretPurpose, SecretRef, SecretResolutionError,
    SecretResolutionRequest,
};
use secret_store::ProjectedSecretProvider;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("prodex-projected-secret-{name}-{stamp}"));
    fs::create_dir_all(&path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    path
}

fn create_private_dir(path: &Path) {
    fs::create_dir(path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

fn write_private(path: &Path, bytes: &[u8]) {
    #[cfg(unix)]
    if path.exists() {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fs::write(path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o440)).unwrap();
    }
}

fn request(version: &str) -> SecretResolutionRequest {
    SecretResolutionRequest::new(
        SecretRef::new("external-secrets", "OPENAI_API_KEY", Some(version)),
        SecretPurpose::ProviderCredential,
    )
}

#[test]
fn production_projected_provider_is_external_versioned_and_rotation_aware() {
    let root = temp_dir("rotation");
    write_private(&root.join("OPENAI_API_KEY"), b"first-value");
    write_private(&root.join("OPENAI_API_KEY.version"), b"v1");
    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();

    let debug = format!("{provider:?}");
    assert!(!debug.contains(&root.display().to_string()));
    assert!(!debug.contains("external-secrets"));
    let descriptor = provider.descriptor();
    assert_eq!(descriptor.kind, SecretProviderKind::ExternalSecretManager);
    assert_eq!(descriptor.name, "external-secrets");
    assert!(descriptor.supports_rotation_without_restart);
    let first = provider.resolve(&request("v1")).unwrap();
    first.with_exposed_secret(|secret| assert_eq!(secret, b"first-value"));
    assert_eq!(first.version(), Some("v1"));

    write_private(&root.join("OPENAI_API_KEY"), b"second-value");
    write_private(&root.join("OPENAI_API_KEY.version"), b"v2");
    assert!(matches!(
        provider.resolve(&request("v1")),
        Err(SecretResolutionError::StaleVersion)
    ));
    provider
        .resolve(&request("v2"))
        .unwrap()
        .with_exposed_secret(|secret| assert_eq!(secret, b"second-value"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn projected_provider_rejects_traversal_and_provider_mismatch() {
    let root = temp_dir("scope");
    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();

    let traversal = SecretResolutionRequest::new(
        SecretRef::new("external-secrets", "../outside", None::<String>),
        SecretPurpose::ProviderCredential,
    );
    assert!(matches!(
        provider.resolve(&traversal),
        Err(SecretResolutionError::PermissionDenied)
    ));
    let wrong_provider = SecretResolutionRequest::new(
        SecretRef::new("vault", "OPENAI_API_KEY", None::<String>),
        SecretPurpose::ProviderCredential,
    );
    assert!(matches!(
        provider.resolve(&wrong_provider),
        Err(SecretResolutionError::NotFound)
    ));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_rejects_symlink_escape_and_world_readable_files() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let root = temp_dir("filesystem");
    let outside = root.with_extension("outside");
    write_private(&outside, b"outside-value");
    symlink(&outside, root.join("escaped")).unwrap();
    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();
    let escaped = SecretResolutionRequest::new(
        SecretRef::new("external-secrets", "escaped", None::<String>),
        SecretPurpose::ProviderCredential,
    );
    assert!(matches!(
        provider.resolve(&escaped),
        Err(SecretResolutionError::PermissionDenied)
    ));

    let actual = root.join("actual");
    write_private(&actual, b"inside-value");
    symlink("actual", root.join("alias")).unwrap();
    let alias = SecretResolutionRequest::new(
        SecretRef::new("external-secrets", "alias", None::<String>),
        SecretPurpose::ProviderCredential,
    );
    assert!(matches!(
        provider.resolve(&alias),
        Err(SecretResolutionError::PermissionDenied)
    ));

    let public = root.join("public");
    fs::write(&public, b"public-value").unwrap();
    fs::set_permissions(&public, fs::Permissions::from_mode(0o444)).unwrap();
    let public_request = SecretResolutionRequest::new(
        SecretRef::new("external-secrets", "public", None::<String>),
        SecretPurpose::ProviderCredential,
    );
    assert!(matches!(
        provider.resolve(&public_request),
        Err(SecretResolutionError::PermissionDenied)
    ));

    let group_writable = root.join("group-writable");
    fs::write(&group_writable, b"group-writable-value").unwrap();
    fs::set_permissions(&group_writable, fs::Permissions::from_mode(0o660)).unwrap();
    let group_writable_request = SecretResolutionRequest::new(
        SecretRef::new("external-secrets", "group-writable", None::<String>),
        SecretPurpose::ProviderCredential,
    );
    assert!(matches!(
        provider.resolve(&group_writable_request),
        Err(SecretResolutionError::PermissionDenied)
    ));
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(outside).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_rejects_writable_parent_directory() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = temp_dir("writable-parent");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o770)).unwrap();
    assert!(ProjectedSecretProvider::new(&root, "external-secrets").is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_follows_atomic_kubernetes_projection_rotation() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("projection");
    let first = root.join("..2026_01");
    create_private_dir(&first);
    write_private(&first.join("OPENAI_API_KEY"), b"first-value");
    write_private(&first.join("OPENAI_API_KEY.version"), b"v1");
    symlink("..2026_01", root.join("..data")).unwrap();
    symlink("..data/OPENAI_API_KEY", root.join("OPENAI_API_KEY")).unwrap();
    symlink(
        "..data/OPENAI_API_KEY.version",
        root.join("OPENAI_API_KEY.version"),
    )
    .unwrap();
    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();
    provider
        .resolve(&request("v1"))
        .unwrap()
        .with_exposed_secret(|secret| assert_eq!(secret, b"first-value"));

    let second = root.join("..2026_02");
    create_private_dir(&second);
    write_private(&second.join("OPENAI_API_KEY"), b"second-value");
    write_private(&second.join("OPENAI_API_KEY.version"), b"v2");
    symlink("..2026_02", root.join("..data.next")).unwrap();
    fs::rename(root.join("..data.next"), root.join("..data")).unwrap();
    provider
        .resolve(&request("v2"))
        .unwrap()
        .with_exposed_secret(|secret| assert_eq!(secret, b"second-value"));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn projected_provider_accepts_only_the_controlled_windows_data_reparse_point() {
    use std::os::windows::fs::symlink_dir;

    let root = temp_dir("windows-projection");
    let generation = root.join("..2026_01");
    create_private_dir(&generation);
    write_private(&generation.join("OPENAI_API_KEY"), b"windows-value");
    write_private(&generation.join("OPENAI_API_KEY.version"), b"v1");
    if let Err(error) = symlink_dir("..2026_01", root.join("..data")) {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            let _ = fs::remove_dir_all(root);
            return;
        }
        panic!("failed to create test data reparse point: {error}");
    }

    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();
    provider
        .resolve(&request("v1"))
        .unwrap()
        .with_exposed_secret(|secret| assert_eq!(secret, b"windows-value"));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_anchors_value_and_version_to_one_kubernetes_generation() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("generation-anchor");
    let first = root.join("..2026_01");
    let second = root.join("..2026_02");
    create_private_dir(&first);
    create_private_dir(&second);
    write_private(&first.join("OPENAI_API_KEY"), b"first-value");
    write_private(&first.join("OPENAI_API_KEY.version"), b"v1");
    write_private(&second.join("OPENAI_API_KEY"), b"second-value");
    write_private(&second.join("OPENAI_API_KEY.version"), b"v2");

    symlink("..2026_02", root.join("..data")).unwrap();
    // Model a value path captured before the ..data swap while version resolves after it.
    symlink("..2026_01/OPENAI_API_KEY", root.join("OPENAI_API_KEY")).unwrap();
    symlink(
        "..data/OPENAI_API_KEY.version",
        root.join("OPENAI_API_KEY.version"),
    )
    .unwrap();

    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();
    let material = provider.resolve(&request("v2")).unwrap();
    material.with_exposed_secret(|secret| assert_eq!(secret, b"second-value"));
    assert_eq!(material.version(), Some("v2"));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_rejects_data_generation_escape() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("generation-escape");
    let outside = root.with_extension("outside-generation");
    create_private_dir(&outside);
    write_private(&outside.join("OPENAI_API_KEY"), b"outside-value");
    write_private(&outside.join("OPENAI_API_KEY.version"), b"v1");
    symlink(&outside, root.join("..data")).unwrap();

    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();
    assert!(matches!(
        provider.resolve(&request("v1")),
        Err(SecretResolutionError::PermissionDenied)
    ));
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_rejects_nested_data_generation_target() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("nested-generation");
    create_private_dir(&root.join("nested"));
    create_private_dir(&root.join("nested/generation"));
    write_private(&root.join("nested/generation/OPENAI_API_KEY"), b"value");
    symlink("nested/generation", root.join("..data")).unwrap();

    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();
    assert!(matches!(
        provider.resolve(&request("v1")),
        Err(SecretResolutionError::PermissionDenied)
    ));
    fs::remove_dir_all(root).unwrap();
}
