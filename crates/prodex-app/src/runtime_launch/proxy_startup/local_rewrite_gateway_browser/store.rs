use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{BrowserResult, MAX_TOKEN_RESPONSE_BYTES, RuntimeGatewayBrowserFailure};
use crate::runtime_launch::proxy_startup::local_rewrite::RuntimeLocalRewriteProxyShared;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;

pub(super) const BROWSER_TRANSACTION_TTL_MS: u64 = 5 * 60 * 1_000;
const MAX_BROWSER_TRANSACTIONS: usize = 1_024;
const TRANSACTION_KEY_PREFIX: &str = "prodex:gateway:browser:transaction:";
const MAX_PROTECTED_ID_TOKEN_BYTES: usize = 128 * 1_024;
const ID_TOKEN_ASSOCIATED_DATA_PREFIX: &str = "prodex:gateway:browser:id-token:v1:";

#[derive(Default)]
pub(crate) struct RuntimeGatewayBrowserState {
    pub(super) transactions: Arc<Mutex<BTreeMap<String, Arc<RuntimeGatewayBrowserTransaction>>>>,
    pub(super) sessions: Arc<Mutex<BTreeMap<String, Arc<RuntimeGatewayBrowserSession>>>>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct RuntimeGatewayBrowserTransaction {
    pub(super) nonce: String,
    pub(super) code_verifier: String,
    pub(super) expires_at_unix_ms: u64,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct RuntimeGatewayBrowserSession {
    pub(super) protected_id_token: String,
    #[serde(default)]
    pub(super) csrf_digest: [u8; 32],
    #[serde(default)]
    pub(super) logout_keys: Vec<String>,
    pub(super) expires_at_unix_ms: u64,
}

pub(super) fn browser_protect_id_token(
    session_id: &str,
    protection_key: &[u8; 32],
    id_token: &str,
) -> BrowserResult<String> {
    if id_token.is_empty() || id_token.len() > MAX_TOKEN_RESPONSE_BYTES {
        return Err(RuntimeGatewayBrowserFailure::Unauthorized);
    }
    let associated_data = format!("{ID_TOKEN_ASSOCIATED_DATA_PREFIX}{session_id}");
    let encrypted = secret_store::encrypt_private_payload(
        protection_key,
        associated_data.as_bytes(),
        id_token.as_bytes(),
    )
    .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(encrypted.as_slice()))
}

pub(super) fn browser_unprotect_id_token(
    session_id: &str,
    protection_key: &[u8; 32],
    protected_id_token: &str,
) -> BrowserResult<zeroize::Zeroizing<Vec<u8>>> {
    if protected_id_token.len() > MAX_PROTECTED_ID_TOKEN_BYTES {
        return Err(RuntimeGatewayBrowserFailure::Unauthorized);
    }
    let encoded = URL_SAFE_NO_PAD
        .decode(protected_id_token)
        .map_err(|_| RuntimeGatewayBrowserFailure::Unauthorized)?;
    let associated_data = format!("{ID_TOKEN_ASSOCIATED_DATA_PREFIX}{session_id}");
    let decrypted =
        secret_store::decrypt_private_payload(protection_key, associated_data.as_bytes(), &encoded)
            .map_err(|_| RuntimeGatewayBrowserFailure::Unauthorized)?;
    if decrypted.is_empty() || decrypted.len() > MAX_TOKEN_RESPONSE_BYTES {
        return Err(RuntimeGatewayBrowserFailure::Unauthorized);
    }
    Ok(decrypted)
}

pub(super) fn browser_store_transaction(
    shared: &RuntimeLocalRewriteProxyShared,
    state: String,
    transaction: RuntimeGatewayBrowserTransaction,
) -> BrowserResult<()> {
    let value = serde_json::to_string(&transaction)
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let transaction = Arc::new(transaction);
    let now = runtime_gateway_unix_epoch_millis();
    let mut transactions = shared
        .gateway_browser
        .transactions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    transactions.retain(|_, transaction| transaction.expires_at_unix_ms > now);
    if transactions.len() >= MAX_BROWSER_TRANSACTIONS {
        return Err(RuntimeGatewayBrowserFailure::Unavailable);
    }
    let previous = transactions.insert(state.clone(), Arc::clone(&transaction));
    drop(transactions);
    let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() else {
        return Ok(());
    };
    match shared
        .runtime_shared
        .async_runtime
        .handle()
        .block_on(executor.put_ephemeral(
            &format!("{TRANSACTION_KEY_PREFIX}{state}"),
            &value,
            Duration::from_millis(BROWSER_TRANSACTION_TTL_MS),
        )) {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => {
            let mut transactions = shared
                .gateway_browser
                .transactions
                .lock()
                .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
            restore_owned_entry(&mut transactions, &state, &transaction, previous);
            Err(RuntimeGatewayBrowserFailure::Unavailable)
        }
    }
}

pub(super) fn browser_take_transaction(
    shared: &RuntimeLocalRewriteProxyShared,
    state: &str,
) -> BrowserResult<Option<RuntimeGatewayBrowserTransaction>> {
    if let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() {
        let local_owner = shared
            .gateway_browser
            .transactions
            .lock()
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?
            .get(state)
            .cloned();
        let value = shared
            .runtime_shared
            .async_runtime
            .handle()
            .block_on(executor.take_ephemeral(&format!("{TRANSACTION_KEY_PREFIX}{state}")))
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
        if let Some(local_owner) = local_owner {
            let mut transactions = shared
                .gateway_browser
                .transactions
                .lock()
                .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
            restore_owned_entry(&mut transactions, state, &local_owner, None);
        }
        return value
            .map(|value| {
                serde_json::from_str(&value).map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)
            })
            .transpose();
    }
    let now = runtime_gateway_unix_epoch_millis();
    let mut transactions = shared
        .gateway_browser
        .transactions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    transactions.retain(|_, transaction| transaction.expires_at_unix_ms > now);
    Ok(transactions
        .remove(state)
        .map(|transaction| transaction.as_ref().clone()))
}

pub(super) fn browser_restore_session_shadow(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: &str,
    owner: &Arc<RuntimeGatewayBrowserSession>,
    previous: Option<Arc<RuntimeGatewayBrowserSession>>,
) -> BrowserResult<()> {
    let mut sessions = shared
        .gateway_browser
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    restore_owned_entry(&mut sessions, session_id, owner, previous);
    Ok(())
}

fn restore_owned_entry<T>(
    entries: &mut BTreeMap<String, Arc<T>>,
    key: &str,
    owner: &Arc<T>,
    previous: Option<Arc<T>>,
) {
    if entries
        .get(key)
        .is_some_and(|current| Arc::ptr_eq(current, owner))
    {
        if let Some(previous) = previous {
            entries.insert(key.to_string(), previous);
        } else {
            entries.remove(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction(nonce: &str) -> Arc<RuntimeGatewayBrowserTransaction> {
        Arc::new(RuntimeGatewayBrowserTransaction {
            nonce: nonce.to_string(),
            code_verifier: format!("verifier-{nonce}"),
            expires_at_unix_ms: 1_000,
        })
    }

    #[test]
    fn browser_shadow_rollback_restores_only_its_owned_entry() {
        let key = "synthetic-state".to_string();
        let previous = transaction("previous");
        let owner = transaction("owner");
        let replacement = transaction("replacement");
        let mut entries = BTreeMap::from([(key.clone(), Arc::clone(&previous))]);

        entries.insert(key.clone(), Arc::clone(&owner));
        restore_owned_entry(&mut entries, &key, &owner, Some(Arc::clone(&previous)));
        assert!(
            entries
                .get(&key)
                .is_some_and(|current| Arc::ptr_eq(current, &previous))
        );

        entries.insert(key.clone(), Arc::clone(&owner));
        entries.insert(key.clone(), Arc::clone(&replacement));
        restore_owned_entry(&mut entries, &key, &owner, None);
        assert!(
            entries
                .get(&key)
                .is_some_and(|current| Arc::ptr_eq(current, &replacement))
        );

        entries.insert(key.clone(), Arc::clone(&owner));
        restore_owned_entry(&mut entries, &key, &owner, None);
        assert!(!entries.contains_key(&key));
    }
}
