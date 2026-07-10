//! Smart-context temporary-disable cooldown state.

use super::*;

const RUNTIME_SMART_CONTEXT_PANIC_COOLDOWN_SECS: u64 = 60;
static RUNTIME_SMART_CONTEXT_DISABLED_UNTIL: OnceLock<Mutex<BTreeMap<PathBuf, u64>>> =
    OnceLock::new();

pub(super) fn runtime_smart_context_disabled_until_for(shared: &RuntimeRotationProxyShared) -> u64 {
    let Some(disabled) = RUNTIME_SMART_CONTEXT_DISABLED_UNTIL.get() else {
        return 0;
    };
    let Ok(disabled) = disabled.lock() else {
        return 0;
    };
    disabled.get(&shared.log_path).copied().unwrap_or_default()
}

pub(super) fn runtime_smart_context_disable_temporarily(
    shared: &RuntimeRotationProxyShared,
    now: u64,
) -> u64 {
    let disabled_until = now.saturating_add(RUNTIME_SMART_CONTEXT_PANIC_COOLDOWN_SECS);
    let disabled = RUNTIME_SMART_CONTEXT_DISABLED_UNTIL.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(mut disabled) = disabled.lock() {
        disabled.insert(shared.log_path.clone(), disabled_until);
    }
    disabled_until
}
