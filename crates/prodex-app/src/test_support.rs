use std::env;
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

// ponytail: Migrate remaining cases to injected readers if this blocks parallel tests.
static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

thread_local! {
    static TEST_ENV_LOCK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) struct TestEnvLockGuard {
    _guard: Option<std::sync::MutexGuard<'static, ()>>,
}

pub(crate) fn acquire_test_env_lock() -> TestEnvLockGuard {
    let guard = TEST_ENV_LOCK_DEPTH.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        if current == 0 {
            Some(
                TEST_ENV_LOCK
                    .get_or_init(|| Mutex::new(()))
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            )
        } else {
            None
        }
    });

    TestEnvLockGuard { _guard: guard }
}

pub(crate) struct TestEnvVarGuard {
    _lock: Option<TestEnvLockGuard>,
    key: Option<&'static str>,
    previous: Option<OsString>,
}

impl TestEnvVarGuard {
    pub(crate) fn lock() -> Self {
        Self {
            _lock: Some(acquire_test_env_lock()),
            key: None,
            previous: None,
        }
    }

    pub(crate) fn set(key: &'static str, value: &str) -> Self {
        let lock = acquire_test_env_lock();
        let previous = env::var_os(key);
        // SAFETY: test env mutation is serialized by the shared env lock guard.
        unsafe { env::set_var(key, value) };
        Self {
            _lock: Some(lock),
            key: Some(key),
            previous,
        }
    }

    pub(crate) fn unset(key: &'static str) -> Self {
        let lock = acquire_test_env_lock();
        let previous = env::var_os(key);
        // SAFETY: test env mutation is serialized by the shared env lock guard.
        unsafe { env::remove_var(key) };
        Self {
            _lock: Some(lock),
            key: Some(key),
            previous,
        }
    }
}

impl Drop for TestEnvVarGuard {
    fn drop(&mut self) {
        if let Some(key) = self.key {
            if let Some(value) = self.previous.as_ref() {
                // SAFETY: test env mutation is serialized by the shared env lock guard.
                unsafe { env::set_var(key, value) };
            } else {
                // SAFETY: test env mutation is serialized by the shared env lock guard.
                unsafe { env::remove_var(key) };
            }
        }
    }
}

impl Drop for TestEnvLockGuard {
    fn drop(&mut self) {
        TEST_ENV_LOCK_DEPTH.with(|depth| {
            let current = depth.get();
            if current > 0 {
                depth.set(current - 1);
            }
        });
    }
}

static TEST_RUNTIME_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

thread_local! {
    static TEST_RUNTIME_LOCK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) struct TestRuntimeLockGuard {
    _guard: Option<std::sync::MutexGuard<'static, ()>>,
}

pub(crate) fn acquire_test_runtime_lock() -> TestRuntimeLockGuard {
    let guard = TEST_RUNTIME_LOCK_DEPTH.with(|depth| {
        let current = depth.get();
        depth.set(current + 1);
        if current == 0 {
            Some(
                TEST_RUNTIME_LOCK
                    .get_or_init(|| Mutex::new(()))
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            )
        } else {
            None
        }
    });

    TestRuntimeLockGuard { _guard: guard }
}

impl Drop for TestRuntimeLockGuard {
    fn drop(&mut self) {
        TEST_RUNTIME_LOCK_DEPTH.with(|depth| {
            let current = depth.get();
            if current > 0 {
                depth.set(current - 1);
            }
        });
    }
}
