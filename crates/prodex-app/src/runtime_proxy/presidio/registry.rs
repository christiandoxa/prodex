//! Bounded per-log-path Presidio runtime state registry.

use crate::RuntimePresidioRedactionConfig;
use anyhow::{Context, Result, anyhow};
use arc_swap::ArcSwap;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

static RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH: OnceLock<RuntimePresidioRedactionRegistry> =
    OnceLock::new();
pub(super) const MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES: usize = 128;

struct RuntimePresidioRedactionRegistry {
    current: ArcSwap<BTreeMap<PathBuf, Arc<RuntimePresidioRedactionState>>>,
    update: Mutex<()>,
}

impl Default for RuntimePresidioRedactionRegistry {
    fn default() -> Self {
        Self {
            current: ArcSwap::from_pointee(BTreeMap::new()),
            update: Mutex::new(()),
        }
    }
}

pub(super) struct RuntimePresidioRedactionState {
    pub(super) config: RuntimePresidioRedactionConfig,
    pub(super) client: reqwest::Client,
    pub(super) slots: Arc<tokio::sync::Semaphore>,
}

impl RuntimePresidioRedactionState {
    pub(super) fn new(config: RuntimePresidioRedactionConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("failed to build Presidio runtime HTTP client")?;
        Ok(Self {
            slots: Arc::new(tokio::sync::Semaphore::new(config.max_concurrency)),
            config,
            client,
        })
    }
}

pub(crate) fn register_runtime_presidio_redaction_proxy_state(
    log_path: &Path,
    config: Option<RuntimePresidioRedactionConfig>,
) -> Result<()> {
    let state = config
        .map(RuntimePresidioRedactionState::new)
        .transpose()?
        .map(Arc::new);
    let registry = RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH.get_or_init(Default::default);
    let _update = registry
        .update
        .lock()
        .map_err(|_| anyhow!("Presidio registry update lock was poisoned"))?;
    let mut snapshot = (**registry.current.load()).clone();
    if let Some(state) = state {
        validate_runtime_presidio_registry_insert(snapshot.len(), snapshot.contains_key(log_path))?;
        snapshot.insert(log_path.to_path_buf(), state);
    } else {
        snapshot.remove(log_path);
    }
    registry.current.store(Arc::new(snapshot));
    Ok(())
}

pub(crate) fn unregister_runtime_presidio_redaction_proxy_state(log_path: &Path) {
    let _ = register_runtime_presidio_redaction_proxy_state(log_path, None);
}

pub(super) fn validate_runtime_presidio_registry_insert(
    entry_count: usize,
    replacing: bool,
) -> Result<()> {
    if !replacing && entry_count >= MAX_RUNTIME_PRESIDIO_REGISTRY_ENTRIES {
        anyhow::bail!("Presidio registry entry limit reached");
    }
    Ok(())
}

pub(super) fn runtime_presidio_redaction_for_log_path(
    log_path: &Path,
) -> Option<Arc<RuntimePresidioRedactionState>> {
    RUNTIME_PRESIDIO_REDACTION_BY_LOG_PATH
        .get()
        .and_then(|registry| registry.current.load().get(log_path).cloned())
}
