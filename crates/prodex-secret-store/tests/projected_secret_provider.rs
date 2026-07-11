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
    let path = std::env::temp_dir().join(format!("prodex-projected-secret-{name}-{stamp}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_private(path: &Path, bytes: &[u8]) {
    if path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        #[cfg(not(unix))]
        {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions).unwrap();
        }
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
fn projected_provider_resolves_versioned_secret_and_observes_rotation() {
    let root = temp_dir("rotation");
    write_private(&root.join("OPENAI_API_KEY"), b"first-value");
    write_private(&root.join("OPENAI_API_KEY.version"), b"v1");
    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();

    let debug = format!("{provider:?}");
    assert!(!debug.contains(&root.display().to_string()));
    assert!(!debug.contains("external-secrets"));
    assert_eq!(
        provider.descriptor().kind,
        SecretProviderKind::ExternalSecretManager
    );
    let first = provider.resolve(&request("v1")).unwrap();
    assert_eq!(first.expose_secret(), b"first-value");
    assert_eq!(first.version(), Some("v1"));

    write_private(&root.join("OPENAI_API_KEY"), b"second-value");
    write_private(&root.join("OPENAI_API_KEY.version"), b"v2");
    assert_eq!(
        provider.resolve(&request("v1")),
        Err(SecretResolutionError::StaleVersion)
    );
    assert_eq!(
        provider.resolve(&request("v2")).unwrap().expose_secret(),
        b"second-value"
    );
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
    assert_eq!(
        provider.resolve(&traversal),
        Err(SecretResolutionError::PermissionDenied)
    );
    let wrong_provider = SecretResolutionRequest::new(
        SecretRef::new("vault", "OPENAI_API_KEY", None::<String>),
        SecretPurpose::ProviderCredential,
    );
    assert_eq!(
        provider.resolve(&wrong_provider),
        Err(SecretResolutionError::NotFound)
    );
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
    assert_eq!(
        provider.resolve(&escaped),
        Err(SecretResolutionError::PermissionDenied)
    );

    let public = root.join("public");
    fs::write(&public, b"public-value").unwrap();
    fs::set_permissions(&public, fs::Permissions::from_mode(0o444)).unwrap();
    let public_request = SecretResolutionRequest::new(
        SecretRef::new("external-secrets", "public", None::<String>),
        SecretPurpose::ProviderCredential,
    );
    assert_eq!(
        provider.resolve(&public_request),
        Err(SecretResolutionError::PermissionDenied)
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_file(outside).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_follows_atomic_kubernetes_projection_rotation() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("projection");
    let first = root.join("..2026_01");
    fs::create_dir(&first).unwrap();
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
    assert_eq!(
        provider.resolve(&request("v1")).unwrap().expose_secret(),
        b"first-value"
    );

    let second = root.join("..2026_02");
    fs::create_dir(&second).unwrap();
    write_private(&second.join("OPENAI_API_KEY"), b"second-value");
    write_private(&second.join("OPENAI_API_KEY.version"), b"v2");
    symlink("..2026_02", root.join("..data.next")).unwrap();
    fs::rename(root.join("..data.next"), root.join("..data")).unwrap();
    assert_eq!(
        provider.resolve(&request("v2")).unwrap().expose_secret(),
        b"second-value"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_anchors_value_and_version_to_one_kubernetes_generation() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("generation-anchor");
    let first = root.join("..2026_01");
    let second = root.join("..2026_02");
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
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
    assert_eq!(material.expose_secret(), b"second-value");
    assert_eq!(material.version(), Some("v2"));
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn projected_provider_rejects_data_generation_escape() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("generation-escape");
    let outside = root.with_extension("outside-generation");
    fs::create_dir(&outside).unwrap();
    write_private(&outside.join("OPENAI_API_KEY"), b"outside-value");
    write_private(&outside.join("OPENAI_API_KEY.version"), b"v1");
    symlink(&outside, root.join("..data")).unwrap();

    let provider = ProjectedSecretProvider::new(&root, "external-secrets").unwrap();
    assert_eq!(
        provider.resolve(&request("v1")),
        Err(SecretResolutionError::PermissionDenied)
    );
    fs::remove_dir_all(root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}
