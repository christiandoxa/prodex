use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_response::RuntimeGatewayAdminError;
use super::local_rewrite_gateway_store_file::{
    RuntimeGatewayStoreFileLoadError, runtime_gateway_virtual_key_store_file_load,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayVirtualKeySource, RuntimeGatewayVirtualKeyStoreFile,
    runtime_gateway_virtual_key_entry_from_stored,
};
use super::*;

mod atomic;
pub(super) use atomic::{
    RuntimeGatewayAdminAtomicWrite, runtime_gateway_mutate_admin_key_store_atomic,
};

fn runtime_gateway_apply_admin_virtual_key_store(
    shared: &RuntimeLocalRewriteProxyShared,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) {
    let Ok(_update) = shared.gateway_credentials.update.lock() else {
        return;
    };
    let current = shared.gateway_credentials.current.load_full();
    let Ok(mut entries) = current.virtual_keys.lock() else {
        return;
    };
    let mut next = entries
        .iter()
        .filter(|entry| entry.source == RuntimeGatewayVirtualKeySource::Policy)
        .cloned()
        .collect::<Vec<_>>();
    let mut seen = next
        .iter()
        .map(|entry| entry.key.name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    for record in &store.keys {
        if seen
            .iter()
            .any(|seen| seen == &record.name.to_ascii_lowercase())
        {
            continue;
        }
        if let Some(entry) = runtime_gateway_virtual_key_entry_from_stored(record) {
            seen.push(entry.key.name.to_ascii_lowercase());
            next.push(entry);
        }
    }
    *entries = next;
}

fn runtime_gateway_virtual_key_store_load_strict_file(
    path: &Path,
) -> Result<RuntimeGatewayVirtualKeyStoreFile, RuntimeGatewayAdminError> {
    runtime_gateway_virtual_key_store_file_load(path).map_err(|err| match err {
        RuntimeGatewayStoreFileLoadError::Invalid(_message) => RuntimeGatewayAdminError::new(
            500,
            "gateway_key_store_invalid",
            "gateway key store is invalid",
        ),
        RuntimeGatewayStoreFileLoadError::Io(_message) => RuntimeGatewayAdminError::new(
            500,
            "gateway_key_store_load_failed",
            "gateway key store could not be loaded",
        ),
    })
}
