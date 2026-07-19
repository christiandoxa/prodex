use crate::{
    AppPaths, PRODEX_SECRET_BACKEND_ENV, PRODEX_SECRET_KEYRING_SERVICE_ENV,
    record_runtime_secret_provider_metric,
};
use anyhow::{Context, Result};
use prodex_observability::{SecretProviderBackend, SecretProviderOperation, SecretProviderResult};
use secret_store::{
    FileSecretBackend, SecretBackend as _, SecretBackendKind, SecretBackendSelection,
    SecretLocation, SecretValue,
};
use std::env;
use std::path::Path;

pub(crate) fn configured_secret_backend_selection() -> Result<SecretBackendSelection> {
    let paths = AppPaths::discover()?;
    configured_secret_backend_selection_for_root(&paths.root)
}

fn configured_secret_backend_selection_for_root(root: &Path) -> Result<SecretBackendSelection> {
    let policy = prodex_runtime_policy::load_runtime_policy_cached(root)?;
    let backend = environment_text(PRODEX_SECRET_BACKEND_ENV)?
        .map(|value| value.parse::<SecretBackendKind>())
        .transpose()
        .map_err(anyhow::Error::new)?
        .or_else(|| policy.as_ref().and_then(|policy| policy.secrets.backend))
        .unwrap_or(SecretBackendKind::File);
    let keyring_service = if backend == SecretBackendKind::Keyring {
        environment_text(PRODEX_SECRET_KEYRING_SERVICE_ENV)?.or_else(|| {
            policy
                .as_ref()
                .and_then(|policy| policy.secrets.keyring_service.clone())
        })
    } else {
        None
    };
    SecretBackendSelection::from_kind(backend, keyring_service).map_err(anyhow::Error::new)
}

pub(crate) fn write_runtime_secret_bounded(
    paths: &AppPaths,
    path: &Path,
    value: SecretValue,
    max_bytes: u64,
) -> Result<()> {
    let selection = configured_secret_backend_selection_for_root(&paths.root)?;
    let location = SecretLocation::file(path);
    let write = selection.write_bounded(&location, value, max_bytes);
    record_runtime_secret_provider_metric(
        operational_backend(selection.kind()),
        SecretProviderOperation::Write,
        if write.is_ok() {
            SecretProviderResult::Success
        } else {
            SecretProviderResult::Failed
        },
    );
    write.context("failed to write runtime secret")?;
    if selection.kind() == SecretBackendKind::Keyring {
        remove_legacy_file_secret(&location);
    }
    Ok(())
}

pub(crate) fn read_runtime_secret_bounded(
    paths: &AppPaths,
    path: &Path,
    max_bytes: u64,
) -> Result<Option<SecretValue>> {
    let selection = configured_secret_backend_selection_for_root(&paths.root)?;
    let location = SecretLocation::file(path);
    let read = selection.read_bounded(&location, max_bytes);
    record_runtime_secret_provider_metric(
        operational_backend(selection.kind()),
        SecretProviderOperation::Read,
        match &read {
            Ok(Some(_)) => SecretProviderResult::Success,
            Ok(None) => SecretProviderResult::NotFound,
            Err(_) => SecretProviderResult::Failed,
        },
    );
    let value = read.context("failed to read runtime secret")?;
    if value.is_some() || selection.kind() != SecretBackendKind::Keyring {
        return Ok(value);
    }

    let Some(legacy) = FileSecretBackend::new()
        .read_bounded(&location, max_bytes)
        .context("failed to read legacy runtime secret")?
    else {
        return Ok(None);
    };
    let migrated = legacy.with_bytes(|bytes| SecretValue::bytes(bytes.to_vec()));
    let migration = selection.write_bounded(&location, migrated, max_bytes);
    record_runtime_secret_provider_metric(
        SecretProviderBackend::Keyring,
        SecretProviderOperation::Write,
        if migration.is_ok() {
            SecretProviderResult::Success
        } else {
            SecretProviderResult::Failed
        },
    );
    migration.context("failed to migrate runtime secret to keyring")?;
    remove_legacy_file_secret(&location);
    Ok(Some(legacy))
}

pub(crate) fn delete_runtime_secret(paths: &AppPaths, path: &Path) {
    let location = SecretLocation::file(path);
    match configured_secret_backend_selection_for_root(&paths.root) {
        Ok(selection) => {
            let deleted = selection.delete(&location);
            record_runtime_secret_provider_metric(
                operational_backend(selection.kind()),
                SecretProviderOperation::Delete,
                if deleted.is_ok() {
                    SecretProviderResult::Success
                } else {
                    SecretProviderResult::Failed
                },
            );
            if selection.kind() == SecretBackendKind::Keyring {
                remove_legacy_file_secret(&location);
            }
        }
        Err(_) => remove_legacy_file_secret(&location),
    }
}

fn operational_backend(kind: SecretBackendKind) -> SecretProviderBackend {
    match kind {
        SecretBackendKind::File => SecretProviderBackend::File,
        SecretBackendKind::Keyring => SecretProviderBackend::Keyring,
    }
}

fn remove_legacy_file_secret(location: &SecretLocation) {
    let backend = FileSecretBackend::new();
    if backend.delete(location).is_err() {
        let _ = backend.remove_untrusted_entry(location);
    }
}

fn environment_text(name: &str) -> Result<Option<String>> {
    env::var_os(name)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| anyhow::anyhow!("{name} must be valid Unicode"))
        })
        .transpose()
}
