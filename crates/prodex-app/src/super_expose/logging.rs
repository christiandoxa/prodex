use anyhow::{Context, Result};
use std::path::PathBuf;

const EXPOSE_LOG_QUEUE_CAPACITY: usize = 256;

#[derive(Clone)]
pub(super) struct ExposeAuditLog {
    path: PathBuf,
    logger: runtime_log::RuntimeAsyncLogger,
}

impl ExposeAuditLog {
    pub(super) fn new() -> Result<Self> {
        let path = crate::create_recorded_runtime_log_path()
            .context("failed to create expose audit log")?;
        let logger = runtime_log::RuntimeAsyncLogger::new_with_recording(
            EXPOSE_LOG_QUEUE_CAPACITY,
            crate::runtime_proxy_format_dropped_log_marker,
            true,
        )
        .context("failed to start expose audit logger")?;
        Ok(Self { path, logger })
    }

    pub(super) fn event<'a>(
        &self,
        event: &str,
        fields: impl IntoIterator<Item = crate::RuntimeProxyLogField<'a>>,
    ) {
        let message = crate::runtime_proxy_structured_log_message(event, fields);
        let line =
            crate::runtime_proxy_format_log_line(&message, crate::runtime_proxy_log_format());
        self.logger.try_enqueue(&self.path, line);
    }

    pub(super) fn flush(&self) -> Result<()> {
        self.logger
            .flush_path(&self.path)
            .context("failed to flush expose audit log")
    }
}
