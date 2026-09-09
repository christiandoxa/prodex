use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{BrowserResult, MAX_TOKEN_RESPONSE_BYTES, RuntimeGatewayBrowserFailure};
use crate::runtime_launch::proxy_startup::local_rewrite::RuntimeLocalRewriteProxyShared;
use crate::runtime_launch::proxy_startup::local_rewrite_gateway_util::runtime_gateway_unix_epoch_millis;

pub(super) const BROWSER_TRANSACTION_TTL_MS: u64 = 5 * 60 * 1_000;
pub(super) const BROWSER_SESSION_TTL_MS: u64 = 8 * 60 * 60 * 1_000;
pub(super) const MAX_BROWSER_TRANSACTIONS: usize = 1_024;
pub(super) const MAX_BROWSER_SESSIONS: usize = 4_096;
const TRANSACTION_KEY_PREFIX: &str = "prodex:gateway:browser:transaction:";
pub(super) const SESSION_KEY_PREFIX: &str = "prodex:gateway:browser:session:";
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

/// Browser shadow persistence boundary; production maps Redis errors to failure and tests stay offline.
pub(super) trait BrowserEphemeralPersistence {
    fn put_ephemeral(&mut self, key: &str, value: &str, ttl: Duration) -> Result<bool, ()>;
    fn add_ephemeral_member(&mut self, key: &str, member: &str, ttl: Duration) -> Result<(), ()>;
    fn delete_ephemeral(&mut self, key: &str) -> Result<(), ()>;
    fn remove_ephemeral_member(&mut self, key: &str, member: &str) -> Result<(), ()>;
}

struct RedisBrowserEphemeralPersistence<'a> {
    handle: &'a tokio::runtime::Handle,
    executor: &'a prodex_storage_redis_runtime::RedisRateLimitExecutor,
}

impl BrowserEphemeralPersistence for RedisBrowserEphemeralPersistence<'_> {
    fn put_ephemeral(&mut self, key: &str, value: &str, ttl: Duration) -> Result<bool, ()> {
        self.handle
            .block_on(self.executor.put_ephemeral(key, value, ttl))
            .map_err(|_| ())
    }

    fn add_ephemeral_member(&mut self, key: &str, member: &str, ttl: Duration) -> Result<(), ()> {
        self.handle
            .block_on(self.executor.add_ephemeral_member(key, member, ttl))
            .map_err(|_| ())
    }

    fn delete_ephemeral(&mut self, key: &str) -> Result<(), ()> {
        self.handle
            .block_on(self.executor.delete_ephemeral(key))
            .map_err(|_| ())
    }

    fn remove_ephemeral_member(&mut self, key: &str, member: &str) -> Result<(), ()> {
        self.handle
            .block_on(self.executor.remove_ephemeral_member(key, member))
            .map_err(|_| ())
    }
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
    let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() else {
        return browser_store_transaction_with_persistence(
            &shared.gateway_browser,
            state,
            transaction,
            now,
            None,
        );
    };
    let mut persistence = RedisBrowserEphemeralPersistence {
        handle: shared.runtime_shared.async_runtime.handle(),
        executor,
    };
    browser_store_transaction_with_persistence(
        &shared.gateway_browser,
        state,
        transaction,
        now,
        Some(&mut persistence),
    )
}

pub(super) fn browser_store_transaction_with_persistence(
    state_store: &RuntimeGatewayBrowserState,
    state: String,
    transaction: RuntimeGatewayBrowserTransaction,
    now: u64,
    persistence: Option<&mut dyn BrowserEphemeralPersistence>,
) -> BrowserResult<()> {
    let value = serde_json::to_string(&transaction)
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let transaction = Arc::new(transaction);
    let mut transactions = state_store
        .transactions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    transactions.retain(|_, transaction| transaction.expires_at_unix_ms > now);
    if transactions.len() >= MAX_BROWSER_TRANSACTIONS {
        return Err(RuntimeGatewayBrowserFailure::Unavailable);
    }
    let previous = transactions.insert(state.clone(), Arc::clone(&transaction));
    drop(transactions);
    let Some(persistence) = persistence else {
        return Ok(());
    };
    match persistence.put_ephemeral(
        &format!("{TRANSACTION_KEY_PREFIX}{state}"),
        &value,
        Duration::from_millis(BROWSER_TRANSACTION_TTL_MS),
    ) {
        Ok(true) => Ok(()),
        Ok(false) | Err(_) => {
            let mut transactions = state_store
                .transactions
                .lock()
                .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
            restore_owned_entry(&mut transactions, &state, &transaction, previous, None);
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
            restore_owned_entry(&mut transactions, state, &local_owner, None, None);
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

pub(super) fn browser_store_session(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: String,
    session: RuntimeGatewayBrowserSession,
) -> BrowserResult<()> {
    let now = runtime_gateway_unix_epoch_millis();
    let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() else {
        return browser_store_session_with_persistence(
            &shared.gateway_browser,
            session_id,
            session,
            now,
            None,
        );
    };
    let mut persistence = RedisBrowserEphemeralPersistence {
        handle: shared.runtime_shared.async_runtime.handle(),
        executor,
    };
    browser_store_session_with_persistence(
        &shared.gateway_browser,
        session_id,
        session,
        now,
        Some(&mut persistence),
    )
}

pub(super) fn browser_store_session_with_persistence(
    state_store: &RuntimeGatewayBrowserState,
    session_id: String,
    session: RuntimeGatewayBrowserSession,
    now: u64,
    persistence: Option<&mut dyn BrowserEphemeralPersistence>,
) -> BrowserResult<()> {
    let value =
        serde_json::to_string(&session).map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    let session = Arc::new(session);
    let mut sessions = state_store
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    sessions.retain(|_, session| session.expires_at_unix_ms > now);
    let evicted = if sessions.len() >= MAX_BROWSER_SESSIONS {
        sessions
            .iter()
            .min_by_key(|(_, session)| session.expires_at_unix_ms)
            .map(|(id, session)| (id.clone(), Arc::clone(session)))
    } else {
        None
    };
    if let Some((oldest, _)) = evicted.as_ref() {
        sessions.remove(oldest);
    }
    let previous = sessions.insert(session_id.clone(), Arc::clone(&session));
    drop(sessions);
    let Some(persistence) = persistence else {
        return Ok(());
    };
    if !matches!(
        persistence.put_ephemeral(
            &format!("{SESSION_KEY_PREFIX}{session_id}"),
            &value,
            Duration::from_millis(BROWSER_SESSION_TTL_MS),
        ),
        Ok(true)
    ) {
        browser_restore_session_shadow(state_store, &session_id, &session, previous, evicted)?;
        return Err(RuntimeGatewayBrowserFailure::Unavailable);
    }
    for logout_key in &session.logout_keys {
        if persistence
            .add_ephemeral_member(
                logout_key,
                &session_id,
                Duration::from_millis(BROWSER_SESSION_TTL_MS),
            )
            .is_err()
        {
            // Redis executor has no compare-delete primitive; retain existing cleanup semantics.
            let _ = persistence.delete_ephemeral(&format!("{SESSION_KEY_PREFIX}{session_id}"));
            for logout_key in &session.logout_keys {
                let _ = persistence.remove_ephemeral_member(logout_key, &session_id);
            }
            browser_restore_session_shadow(state_store, &session_id, &session, previous, evicted)?;
            return Err(RuntimeGatewayBrowserFailure::Unavailable);
        }
    }
    Ok(())
}

pub(super) fn browser_load_session(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: &str,
) -> BrowserResult<Option<RuntimeGatewayBrowserSession>> {
    if let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() {
        let value = shared
            .runtime_shared
            .async_runtime
            .handle()
            .block_on(executor.get_ephemeral(&format!("{SESSION_KEY_PREFIX}{session_id}")))
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
        return value
            .map(|value| {
                serde_json::from_str(&value).map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)
            })
            .transpose();
    }
    let now = runtime_gateway_unix_epoch_millis();
    let mut sessions = shared
        .gateway_browser
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    sessions.retain(|_, session| session.expires_at_unix_ms > now);
    Ok(sessions
        .get(session_id)
        .map(|session| session.as_ref().clone()))
}

pub(super) fn browser_delete_session(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: &str,
) -> BrowserResult<()> {
    let session = browser_load_session(shared, session_id)?;
    browser_delete_session_record(shared, session_id, session.as_ref())
}

pub(super) fn browser_delete_session_record(
    shared: &RuntimeLocalRewriteProxyShared,
    session_id: &str,
    session: Option<&RuntimeGatewayBrowserSession>,
) -> BrowserResult<()> {
    let local_owner = shared
        .gateway_browser
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?
        .get(session_id)
        .cloned();
    if let Some(executor) = shared.gateway_redis_rate_limit_executor.as_ref() {
        // Redis executor has no compare-delete primitive; retain existing cleanup semantics.
        shared
            .runtime_shared
            .async_runtime
            .handle()
            .block_on(executor.delete_ephemeral(&format!("{SESSION_KEY_PREFIX}{session_id}")))
            .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
        if let Some(session) = session {
            for logout_key in &session.logout_keys {
                shared
                    .runtime_shared
                    .async_runtime
                    .handle()
                    .block_on(executor.remove_ephemeral_member(logout_key, session_id))
                    .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
            }
        }
    }
    if let Some(local_owner) = local_owner {
        browser_restore_session_shadow(
            &shared.gateway_browser,
            session_id,
            &local_owner,
            None,
            None,
        )?;
    }
    Ok(())
}

fn browser_restore_session_shadow(
    state_store: &RuntimeGatewayBrowserState,
    session_id: &str,
    owner: &Arc<RuntimeGatewayBrowserSession>,
    previous: Option<Arc<RuntimeGatewayBrowserSession>>,
    evicted: Option<(String, Arc<RuntimeGatewayBrowserSession>)>,
) -> BrowserResult<()> {
    let mut sessions = state_store
        .sessions
        .lock()
        .map_err(|_| RuntimeGatewayBrowserFailure::Unavailable)?;
    restore_owned_entry(&mut sessions, session_id, owner, previous, evicted);
    Ok(())
}

fn restore_owned_entry<T>(
    entries: &mut BTreeMap<String, Arc<T>>,
    key: &str,
    owner: &Arc<T>,
    previous: Option<Arc<T>>,
    evicted: Option<(String, Arc<T>)>,
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
        if let Some((evicted_key, evicted)) = evicted {
            let _ = entries.entry(evicted_key).or_insert(evicted);
        }
    }
}
