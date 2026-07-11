use super::*;
use std::sync::{Mutex, OnceLock};

static TEST_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

thread_local! {
    static TEST_ENV_LOCK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct TestEnvLockGuard {
    _guard: Option<std::sync::MutexGuard<'static, ()>>,
}

fn acquire_test_env_lock() -> TestEnvLockGuard {
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

pub(super) struct TestEnvVarGuard {
    _lock: Option<TestEnvLockGuard>,
    key: Option<&'static str>,
    previous: Option<OsString>,
}

impl TestEnvVarGuard {
    pub(super) fn lock() -> Self {
        Self {
            _lock: Some(acquire_test_env_lock()),
            key: None,
            previous: None,
        }
    }

    pub(super) fn set(key: &'static str, value: &str) -> Self {
        let lock = acquire_test_env_lock();
        let previous = env::var_os(key);
        unsafe { env::set_var(key, value) };
        Self {
            _lock: Some(lock),
            key: Some(key),
            previous,
        }
    }

    pub(super) fn unset(key: &'static str) -> Self {
        let lock = acquire_test_env_lock();
        let previous = env::var_os(key);
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
                unsafe { env::set_var(key, value) };
            } else {
                unsafe { env::remove_var(key) };
            }
        }
    }
}

pub(super) fn test_profile(path: &str) -> prodex_state::ProfileEntry {
    prodex_state::ProfileEntry {
        codex_home: PathBuf::from(path),
        managed: true,
        email: None,
        provider: prodex_state::ProfileProvider::Openai,
    }
}
