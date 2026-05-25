use std::any::Any;
use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock};

static RUNTIME_PANIC_HOOK_INSTALL_LOCK: Mutex<()> = Mutex::new(());
static RUNTIME_PANIC_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();

thread_local! {
    static RUNTIME_SUPPRESS_PANIC_HOOK: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn catch_runtime_unwind_silently<F, T>(f: F) -> Result<T, Box<dyn Any + Send + 'static>>
where
    F: FnOnce() -> T,
{
    install_runtime_panic_hook_suppression();
    let previous_suppression = RUNTIME_SUPPRESS_PANIC_HOOK.with(|suppressed| {
        let previous = suppressed.get();
        suppressed.set(true);
        previous
    });
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    RUNTIME_SUPPRESS_PANIC_HOOK.with(|suppressed| suppressed.set(previous_suppression));
    result
}

pub(crate) fn runtime_panic_payload_label(payload: &(dyn Any + Send)) -> String {
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

fn install_runtime_panic_hook_suppression() {
    if RUNTIME_PANIC_HOOK_INSTALLED.get().is_some() {
        return;
    }
    let Ok(_guard) = RUNTIME_PANIC_HOOK_INSTALL_LOCK.lock() else {
        return;
    };
    if RUNTIME_PANIC_HOOK_INSTALLED.get().is_some() {
        return;
    }
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if RUNTIME_SUPPRESS_PANIC_HOOK.with(Cell::get) {
            return;
        }
        previous(info);
    }));
    let _ = RUNTIME_PANIC_HOOK_INSTALLED.set(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_unwind_guard_catches_panic_payload() {
        let result = catch_runtime_unwind_silently(|| panic!("runtime guard boom"));

        let panic = result.expect_err("runtime panic should be caught");
        assert_eq!(
            runtime_panic_payload_label(panic.as_ref()),
            "runtime guard boom"
        );
    }
}
