use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{BrowserResult, RuntimeGatewayBrowserFailure};
use crate::runtime_launch::proxy_startup::local_rewrite::RuntimeLocalRewriteProxyShared;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;

pub(super) const BROWSER_TRANSACTION_TTL_MS: u64 = 5 * 60 * 1_000;
const MAX_BROWSER_TRANSACTIONS: usize = 1_024;
const TRANSACTION_KEY_PREFIX: &str = "prodex:gateway:browser:transaction:";

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
    pub(super) id_token: String,
    #[serde(default)]
    pub(super) logout_keys: Vec<String>,
    pub(super) expires_at_unix_ms: u64,
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
