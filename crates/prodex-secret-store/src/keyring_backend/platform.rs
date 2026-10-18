//! Explicit maintained OS stores without the retired standalone zvariant stack.
//! Secret Service keeps the same default collection, service/username search
//! attributes and binary secret envelope. It does not migrate or enumerate data.
use keyring_core::{CredentialStore, Entry, Error, Result};
use std::sync::{Arc, LazyLock};

static STORE: LazyLock<Result<Arc<CredentialStore>>> = LazyLock::new(open_store);

pub(super) fn entry(service: &str, user: &str) -> Result<Entry> {
    let store = STORE.as_ref().map_err(|_| Error::NoDefaultStore)?;
    store.build(service, user, None)
}

#[cfg(target_os = "macos")]
fn open_store() -> Result<Arc<CredentialStore>> {
    Ok(apple_native_keyring_store::keychain::Store::new()?)
}

#[cfg(target_os = "windows")]
fn open_store() -> Result<Arc<CredentialStore>> {
    Ok(windows_native_keyring_store::Store::new()?)
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
fn open_store() -> Result<Arc<CredentialStore>> {
    // Use Rust cryptography and the bundled libdbus, not an OpenSSL dependency
    // or an unencrypted/plaintext fallback when Secret Service is unavailable.
    Ok(dbus_secret_service_keyring_store::Store::new()?)
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )
)))]
fn open_store() -> Result<Arc<CredentialStore>> {
    Err(Error::NoDefaultStore)
}
