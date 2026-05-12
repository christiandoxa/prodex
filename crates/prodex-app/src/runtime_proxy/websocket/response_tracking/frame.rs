use super::*;

pub(super) fn mark_runtime_websocket_upstream_frame_seen(
    upstream_socket: &mut RuntimeUpstreamWebSocket,
    first_upstream_frame_seen: &mut bool,
) -> Result<()> {
    if *first_upstream_frame_seen {
        return Ok(());
    }
    *first_upstream_frame_seen = true;
    runtime_set_upstream_websocket_io_timeout(
        upstream_socket,
        Some(Duration::from_millis(
            runtime_proxy_websocket_precommit_progress_timeout_ms(),
        )),
    )
    .context("failed to restore runtime websocket upstream timeout")
}
