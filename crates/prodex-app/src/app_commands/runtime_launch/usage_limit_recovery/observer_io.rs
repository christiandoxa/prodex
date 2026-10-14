//! An observer must not terminate a healthy child on temporary host resource
//! pressure. This layer classifies OS/SQLite errors only, not recovery policy.
use super::*;

impl GoalUsageLimitMonitor {
    pub(crate) fn take_usage_limit_signal(&mut self) -> Result<Option<String>> {
        if self.observer_retry_at.is_some_and(|at| Instant::now() < at) {
            return Ok(None);
        }
        match self.poll_usage_limit_signal() {
            Ok(signal) => {
                self.observer_retry_at = None;
                Ok(signal)
            }
            Err(error) if observer_error_is_transient(&error) => {
                // Release our optional SQLite handles, never foreign descriptors.
                // Keep the session identity, evidence and unread rollout cursor.
                // Unknown observer state authorizes neither replay nor rotation.
                self.connection = None;
                self.observer_retry_at = Some(Instant::now() + GOAL_USAGE_LIMIT_RETRY_INTERVAL);
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

fn observer_error_is_transient(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        if let Some(error) = cause.downcast_ref::<std::io::Error>() {
            if matches!(
                error.kind(),
                std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
            ) {
                return true;
            }
            #[cfg(unix)]
            if matches!(error.raw_os_error(), Some(libc::EMFILE | libc::ENFILE)) {
                return true;
            }
            #[cfg(windows)]
            if error.raw_os_error() == Some(4) {
                // ERROR_TOO_MANY_OPEN_FILES
                return true;
            }
        }
        matches!(cause.downcast_ref::<rusqlite::Error>(),
            Some(rusqlite::Error::SqliteFailure(code, _)) if matches!(code.code,
                rusqlite::ErrorCode::CannotOpen | rusqlite::ErrorCode::SystemIoFailure
                | rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_only_retries_typed_resource_errors_not_invalid_identity() {
        let retryable = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::WouldBlock))
            .context("synthetic marker read");
        assert!(observer_error_is_transient(&retryable));
        let denied = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert!(!observer_error_is_transient(&denied));
        assert!(!observer_error_is_transient(&anyhow::anyhow!(
            "invalid runtime goal session id"
        )));
        #[cfg(unix)]
        for code in [libc::EMFILE, libc::ENFILE] {
            assert!(observer_error_is_transient(
                &anyhow::Error::new(std::io::Error::from_raw_os_error(code))
                    .context("synthetic marker read")
            ));
        }
    }
}
