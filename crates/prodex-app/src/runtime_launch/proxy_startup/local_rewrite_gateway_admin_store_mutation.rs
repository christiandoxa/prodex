use super::RuntimeGatewayStateStore;
use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_response::RuntimeGatewayAdminError;
use super::local_rewrite_gateway_backend_connection::{
    runtime_gateway_postgres_open, runtime_gateway_redis_with_lock, runtime_gateway_sqlite_open,
};
use super::local_rewrite_gateway_key_store_backend::{
    runtime_gateway_postgres_load_key_store_from_client,
    runtime_gateway_postgres_save_key_store_in_tx, runtime_gateway_redis_load_key_store_from_conn,
    runtime_gateway_redis_save_key_store, runtime_gateway_sqlite_load_key_store_from_conn,
    runtime_gateway_sqlite_save_key_store_in_tx,
};
use super::local_rewrite_gateway_store_file::{
    RuntimeGatewayStoreFileLoadError, runtime_gateway_virtual_key_store_file_load,
    runtime_gateway_virtual_key_store_file_save,
};
use super::local_rewrite_gateway_store_types::{
    RuntimeGatewayVirtualKeySource, RuntimeGatewayVirtualKeyStoreFile,
    runtime_gateway_virtual_key_entry_from_stored, runtime_gateway_virtual_key_store_version,
};
use super::*;
use fs2::FileExt;
use rusqlite::TransactionBehavior;
use std::fs::OpenOptions;

pub(super) fn runtime_gateway_mutate_admin_key_store<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    match &shared.gateway_state_store {
        RuntimeGatewayStateStore::File { key_store_path, .. } => {
            runtime_gateway_mutate_admin_key_store_file(shared, key_store_path, mutation)
        }
        RuntimeGatewayStateStore::Sqlite { path } => {
            runtime_gateway_mutate_admin_key_store_sqlite(shared, path, mutation)
        }
        RuntimeGatewayStateStore::Postgres { url, .. } => {
            runtime_gateway_mutate_admin_key_store_postgres(shared, url, mutation)
        }
        RuntimeGatewayStateStore::Redis { url, .. } => {
            runtime_gateway_mutate_admin_key_store_redis(shared, url, mutation)
        }
    }
}

fn runtime_gateway_mutate_admin_key_store_file<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &Path,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let lock_path = path.with_extension("json.lock");
    if let Some(parent) = lock_path.parent()
        && let Err(_err) = std::fs::create_dir_all(parent)
    {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_lock_failed",
            "gateway key store lock could not be acquired",
        ));
    }
    let lock_file = match OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
    {
        Ok(file) => file,
        Err(_err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_lock_failed",
                "gateway key store lock could not be acquired",
            ));
        }
    };
    if let Err(_err) = lock_file.lock_exclusive() {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_lock_failed",
            "gateway key store lock could not be acquired",
        ));
    }
    let mut store = match runtime_gateway_virtual_key_store_load_strict_file(path) {
        Ok(store) => store,
        Err(err) => {
            let _ = lock_file.unlock();
            return Err(err.into_response());
        }
    };
    store.version = runtime_gateway_virtual_key_store_version();
    if let Err(err) = mutation(&mut store) {
        let _ = lock_file.unlock();
        return Err(err.into_response());
    }
    store.sort_for_rendering();
    if let Err(_err) = runtime_gateway_virtual_key_store_file_save(path, &store) {
        let _ = lock_file.unlock();
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            "gateway key store could not be saved",
        ));
    }
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    let _ = lock_file.unlock();
    Ok(())
}

fn runtime_gateway_mutate_admin_key_store_sqlite<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    path: &Path,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let mut conn = match runtime_gateway_sqlite_open(path) {
        Ok(conn) => conn,
        Err(_err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_load_failed",
                "gateway key store could not be loaded",
            ));
        }
    };
    let tx = match conn.transaction_with_behavior(TransactionBehavior::Immediate) {
        Ok(tx) => tx,
        Err(_err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_lock_failed",
                "gateway key store lock could not be acquired",
            ));
        }
    };
    let mut store = match runtime_gateway_sqlite_load_key_store_from_conn(&tx) {
        Ok(store) => store,
        Err(_err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_load_failed",
                "gateway key store could not be loaded",
            ));
        }
    };
    store.version = runtime_gateway_virtual_key_store_version();
    if let Err(err) = mutation(&mut store) {
        return Err(err.into_response());
    }
    store.sort_keys();
    if let Err(_err) = runtime_gateway_sqlite_save_key_store_in_tx(&tx, &store) {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            "gateway key store could not be saved",
        ));
    }
    if let Err(_err) = tx.commit() {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            "gateway key store could not be saved",
        ));
    }
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    Ok(())
}

fn runtime_gateway_mutate_admin_key_store_postgres<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let mut client = match runtime_gateway_postgres_open(url) {
        Ok(client) => client,
        Err(_err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_load_failed",
                "gateway key store could not be loaded",
            ));
        }
    };
    let mut tx = match client.transaction() {
        Ok(tx) => tx,
        Err(_err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_lock_failed",
                "gateway key store lock could not be acquired",
            ));
        }
    };
    if let Err(_err) = tx.batch_execute(
        "LOCK TABLE prodex_gateway_virtual_keys, prodex_gateway_scim_users IN EXCLUSIVE MODE",
    ) {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_lock_failed",
            "gateway key store lock could not be acquired",
        ));
    }
    let mut store = match runtime_gateway_postgres_load_key_store_from_client(&mut tx) {
        Ok(store) => store,
        Err(_err) => {
            return Err(build_runtime_proxy_json_error_response(
                500,
                "gateway_key_store_load_failed",
                "gateway key store could not be loaded",
            ));
        }
    };
    store.version = runtime_gateway_virtual_key_store_version();
    if let Err(err) = mutation(&mut store) {
        return Err(err.into_response());
    }
    store.sort_for_rendering();
    if let Err(_err) = runtime_gateway_postgres_save_key_store_in_tx(&mut tx, &store) {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            "gateway key store could not be saved",
        ));
    }
    if let Err(_err) = tx.commit() {
        return Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            "gateway key store could not be saved",
        ));
    }
    runtime_gateway_apply_admin_virtual_key_store(shared, &store);
    Ok(())
}

fn runtime_gateway_mutate_admin_key_store_redis<F>(
    shared: &RuntimeLocalRewriteProxyShared,
    url: &str,
    mutation: F,
) -> Result<(), tiny_http::ResponseBox>
where
    F: FnOnce(&mut RuntimeGatewayVirtualKeyStoreFile) -> Result<(), RuntimeGatewayAdminError>,
{
    let result = runtime_gateway_redis_with_lock(
        url,
        super::local_rewrite::RUNTIME_GATEWAY_REDIS_KEY_STORE_LOCK,
        super::local_rewrite::runtime_gateway_generate_virtual_key_token,
        |conn| -> Result<Result<RuntimeGatewayVirtualKeyStoreFile, RuntimeGatewayAdminError>> {
            let mut store = runtime_gateway_redis_load_key_store_from_conn(
                conn,
                super::local_rewrite::RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY,
            )?;
            store.version = runtime_gateway_virtual_key_store_version();
            if let Err(err) = mutation(&mut store) {
                return Ok(Err(err));
            }
            store.sort_for_rendering();
            runtime_gateway_redis_save_key_store(
                conn,
                super::local_rewrite::RUNTIME_GATEWAY_REDIS_KEY_STORE_KEY,
                &store,
            )?;
            Ok(Ok(store))
        },
    );
    match result {
        Ok(Ok(store)) => {
            runtime_gateway_apply_admin_virtual_key_store(shared, &store);
            Ok(())
        }
        Ok(Err(err)) => Err(err.into_response()),
        Err(_err) => Err(build_runtime_proxy_json_error_response(
            500,
            "gateway_key_store_save_failed",
            "gateway key store could not be saved",
        )),
    }
}

fn runtime_gateway_apply_admin_virtual_key_store(
    shared: &RuntimeLocalRewriteProxyShared,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) {
    let Ok(mut entries) = shared.gateway_virtual_keys.lock() else {
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
