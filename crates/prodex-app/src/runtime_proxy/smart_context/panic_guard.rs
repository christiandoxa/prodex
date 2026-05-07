use super::{
    RuntimeRotationProxyShared, RuntimeRouteKind, RuntimeSmartContextTransport, runtime_proxy_log,
    runtime_proxy_log_field, runtime_proxy_structured_log_message, runtime_route_kind_label,
};
use std::cell::Cell;
use std::sync::{Arc, Mutex};

static RUNTIME_SMART_CONTEXT_PANIC_HOOK_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static RUNTIME_SMART_CONTEXT_SUPPRESS_PANIC_HOOK: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug)]
pub(super) struct RuntimeSmartContextInjectedPanic;

pub(super) type RuntimeSmartContextPanicHook =
    Box<dyn Fn(&std::panic::PanicHookInfo<'_>) + Sync + Send + 'static>;
type RuntimeSmartContextSharedPanicHook = Arc<Mutex<Option<RuntimeSmartContextPanicHook>>>;

struct RuntimeSmartContextPanicHookSuppression {
    previous: RuntimeSmartContextSharedPanicHook,
    previous_suppression: bool,
}

impl RuntimeSmartContextPanicHookSuppression {
    fn enter() -> Self {
        let previous_suppression = RUNTIME_SMART_CONTEXT_SUPPRESS_PANIC_HOOK.with(|suppressed| {
            let previous = suppressed.get();
            suppressed.set(true);
            previous
        });
        let previous = std::panic::take_hook();
        let previous = Arc::new(Mutex::new(Some(previous)));
        let hook_previous = Arc::clone(&previous);
        std::panic::set_hook(Box::new(move |info| {
            if RUNTIME_SMART_CONTEXT_SUPPRESS_PANIC_HOOK.with(Cell::get) {
                return;
            }
            if let Ok(previous) = hook_previous.lock()
                && let Some(previous) = previous.as_ref()
            {
                previous(info);
            }
        }));
        Self {
            previous,
            previous_suppression,
        }
    }
}

impl Drop for RuntimeSmartContextPanicHookSuppression {
    fn drop(&mut self) {
        let _installed_hook = std::panic::take_hook();
        if let Ok(mut previous) = self.previous.lock()
            && let Some(previous) = previous.take()
        {
            std::panic::set_hook(previous);
        }
        RUNTIME_SMART_CONTEXT_SUPPRESS_PANIC_HOOK
            .with(|suppressed| suppressed.set(self.previous_suppression));
    }
}

pub(super) fn catch_runtime_smart_context_unwind_silently<F, T>(
    f: F,
) -> Result<T, Box<dyn std::any::Any + Send + 'static>>
where
    F: FnOnce() -> T,
{
    let Ok(_hook_lock) = RUNTIME_SMART_CONTEXT_PANIC_HOOK_LOCK.lock() else {
        return std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    };
    let _suppression = RuntimeSmartContextPanicHookSuppression::enter();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
}

fn runtime_smart_context_panic_payload_label(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<Box<str>>() {
        return message.to_string();
    }
    "non_string_panic".to_string()
}

pub(super) fn runtime_smart_context_log_panic(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
    body_bytes: usize,
    panic: &(dyn std::any::Any + Send),
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "smart_context_panic",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport.label()),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("profile", profile_name.unwrap_or("-")),
                runtime_proxy_log_field("panic", runtime_smart_context_panic_payload_label(panic)),
                runtime_proxy_log_field("decision", "pass_through"),
                runtime_proxy_log_field("body_bytes", body_bytes.to_string()),
            ],
        ),
    );
}

pub(super) fn runtime_smart_context_log_prepare_fallback(
    request_id: u64,
    shared: &RuntimeRotationProxyShared,
    route_kind: RuntimeRouteKind,
    transport: RuntimeSmartContextTransport,
    profile_name: Option<&str>,
    body_bytes: usize,
    reason: &str,
) {
    runtime_proxy_log(
        shared,
        runtime_proxy_structured_log_message(
            "smart_context_prepare_fallback",
            [
                runtime_proxy_log_field("request", request_id.to_string()),
                runtime_proxy_log_field("transport", transport.label()),
                runtime_proxy_log_field("route", runtime_route_kind_label(route_kind)),
                runtime_proxy_log_field("profile", profile_name.unwrap_or("-")),
                runtime_proxy_log_field("reason", reason),
                runtime_proxy_log_field("decision", "pass_through"),
                runtime_proxy_log_field("body_bytes", body_bytes.to_string()),
            ],
        ),
    );
}
