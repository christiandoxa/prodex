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
    pub(super) transactions: Arc<Mutex<BTreeMap<String, RuntimeGatewayBrowserTransaction>>>,
    pub(super) sessions: Arc<Mutex<BTreeMap<String, RuntimeGatewayBrowserSession>>>,
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
    transactions.insert(state.clone(), transaction.clone());
    drop(transactions);
    let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() else {
        return Ok(());
    };
    let value = serde_json::to_string(&transaction)
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let stored = shared
        .runtime_shared
        .async_runtime
        .handle()
        .block_on(executor.put_ephemeral(
            &format!("{TRANSACTION_KEY_PREFIX}{state}"),
            &value,
            Duration::from_millis(BROWSER_TRANSACTION_TTL_MS),
        ))
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    if !stored {
        shared
            .gateway_browser
            .transactions
            .lock()
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?
            .remove(&state);
        return Err(RuntimeGatewayBrowserFailure::Unavailable);
    }
    Ok(())
}

pub(super) fn browser_take_transaction(
    shared: &RuntimeLocalRewriteProxyShared,
    state: &str,
) -> BrowserResult<Option<RuntimeGatewayBrowserTransaction>> {
    if let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() {
        let value = shared
            .runtime_shared
            .async_runtime
            .handle()
            .block_on(executor.take_ephemeral(&format!("{TRANSACTION_KEY_PREFIX}{state}")))
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
        shared
            .gateway_browser
            .transactions
            .lock()
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?
            .remove(state);
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
    Ok(transactions.remove(state))
}
