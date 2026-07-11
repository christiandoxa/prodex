use prodex_domain::{
    SecretProvider, SecretProviderKind, SecretPurpose, SecretRef, SecretResolutionError,
    SecretResolutionRequest,
};
use secret_store::{DEVELOPMENT_SECRET_PROVIDER_NAME, DevelopmentSecretProvider, SecretError};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "prodex-development-secret-{name}-{}-{stamp:x}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn request(provider: &str, name: &str, version: Option<&str>) -> SecretResolutionRequest {
    SecretResolutionRequest::new(
        SecretRef::new(provider, name, version),
        SecretPurpose::ProviderCredential,
    )
}

fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o440)).unwrap();
    }
}

struct EnvGuard {
    name: String,
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set(name: &str, value: &str) -> Self {
        let previous = std::env::var_os(name);
        unsafe { std::env::set_var(name, value) };
        Self {
            name: name.to_string(),
            previous,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => unsafe { std::env::set_var(&self.name, value) },
            None => unsafe { std::env::remove_var(&self.name) },
        }
    }
}

#[test]
fn development_provider_resolves_environment_reference_with_exact_provider_name() {
    let root = temp_dir("environment");
    let _environment = EnvGuard::set("PRODEX_TEST_DEVELOPMENT_SECRET", "development-token");
    let provider = DevelopmentSecretProvider::for_development(&root).unwrap();

    assert_eq!(provider.provider_name(), DEVELOPMENT_SECRET_PROVIDER_NAME);
    assert_eq!(
        provider.descriptor().kind,
        SecretProviderKind::DevelopmentEnvFile
    );
    assert!(!provider.descriptor().supports_rotation_without_restart);
    assert_eq!(
        provider
            .resolve(&request(
                DEVELOPMENT_SECRET_PROVIDER_NAME,
                "env:PRODEX_TEST_DEVELOPMENT_SECRET",
                None,
            ))
            .unwrap()
            .expose_secret(),
        b"development-token"
    );
    assert_eq!(
        provider.resolve(&request(
            "Development",
            "env:PRODEX_TEST_DEVELOPMENT_SECRET",
            None
        )),
        Err(SecretResolutionError::NotFound)
    );

    let rendered = format!("{provider:?}");
    assert!(!rendered.contains("development-token"));
    assert!(!rendered.contains("PRODEX_TEST_DEVELOPMENT_SECRET"));
    assert!(!rendered.contains(&root.display().to_string()));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn development_provider_resolves_private_file_below_root() {
    let root = temp_dir("file");
    let path = root.join("nested/token");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    write_private(&path, b"file-token");
    let provider = DevelopmentSecretProvider::for_development(&root).unwrap();

    let material = provider
        .resolve(&request(
            DEVELOPMENT_SECRET_PROVIDER_NAME,
            "file:nested/token",
            None,
        ))
        .unwrap();
    assert_eq!(material.expose_secret(), b"file-token");
    assert_eq!(material.version(), None);
    assert_eq!(
        provider.resolve(&request(
            DEVELOPMENT_SECRET_PROVIDER_NAME,
            "file:nested/token",
            Some("v1"),
        )),
        Err(SecretResolutionError::StaleVersion)
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn development_provider_rejects_unknown_reference_schemes_and_missing_environment() {
    let root = temp_dir("references");
    let provider = DevelopmentSecretProvider::for_development(&root).unwrap();

    for name in [
        "env:PRODEX_TEST_DEVELOPMENT_SECRET_MISSING",
        "env:PRODEX_TEST_DEVELOPMENT_SECRET-NOT-VALID",
        "file:",
        "file:../outside",
        "file:/tmp/outside",
        "https://example.invalid/secret",
    ] {
        let expected = if name == "env:PRODEX_TEST_DEVELOPMENT_SECRET-NOT-VALID"
            || name == "file:"
            || name == "https://example.invalid/secret"
        {
            SecretResolutionError::PermissionDenied
        } else if name.starts_with("env:") {
            SecretResolutionError::NotFound
        } else {
            SecretResolutionError::PermissionDenied
        };
        assert_eq!(
            provider.resolve(&request(DEVELOPMENT_SECRET_PROVIDER_NAME, name, None)),
            Err(expected),
            "unexpected result for reference kind {name}"
        );
    }

    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn development_provider_rejects_file_escape_public_and_oversized_inputs() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};

    let root = temp_dir("file-rules");
    let outside = root.with_extension("outside");
    write_private(&outside, b"outside-token");
    symlink(&outside, root.join("escaped")).unwrap();
    let public = root.join("public");
    fs::write(&public, b"public-token").unwrap();
    fs::set_permissions(&public, fs::Permissions::from_mode(0o444)).unwrap();
    let oversized = root.join("oversized");
    let file = fs::File::create(&oversized).unwrap();
    file.set_len(64 * 1024 + 1).unwrap();
    fs::set_permissions(&oversized, fs::Permissions::from_mode(0o440)).unwrap();
    let provider = DevelopmentSecretProvider::for_development(&root).unwrap();

    for (name, expected) in [
        ("file:escaped", SecretResolutionError::PermissionDenied),
        ("file:public", SecretResolutionError::PermissionDenied),
        ("file:oversized", SecretResolutionError::ProviderUnavailable),
    ] {
        assert_eq!(
            provider.resolve(&request(DEVELOPMENT_SECRET_PROVIDER_NAME, name, None)),
            Err(expected)
        );
    }

    fs::remove_dir_all(root).unwrap();
    fs::remove_file(outside).unwrap();
}

#[test]
fn development_provider_constructor_and_errors_are_redacted() {
    let root = temp_dir("redaction");
    let sensitive_name = "development-sensitive-provider";
    let error = DevelopmentSecretProvider::new(&root, "provider name with spaces").unwrap_err();
    assert!(matches!(error, SecretError::InvalidLocation { .. }));
    assert!(!error.to_string().contains(sensitive_name));
    assert!(!error.to_string().contains("provider name"));

    let provider = DevelopmentSecretProvider::new(&root, sensitive_name).unwrap();
    let rendered = format!("{provider:?}");
    assert!(!rendered.contains(sensitive_name));
    assert!(!rendered.contains(&root.display().to_string()));
    assert_eq!(
        provider.resolve(&request(sensitive_name, "env:MISSING", None)),
        Err(SecretResolutionError::NotFound)
    );

    fs::remove_dir_all(root).unwrap();
}
