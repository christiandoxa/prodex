use super::*;

#[path = "runtime_proxy_continuations/websocket_precommit.rs"]
mod websocket_precommit;
#[path = "runtime_proxy_continuations/websocket_pool_exhaustion.rs"]
mod websocket_pool_exhaustion;
#[path = "runtime_proxy_continuations/websocket_recovery.rs"]
mod websocket_recovery;
#[path = "runtime_proxy_continuations/websocket_invalid_previous_response.rs"]
mod websocket_invalid_previous_response;
#[path = "runtime_proxy_continuations/http_followups.rs"]
mod http_followups;
#[path = "runtime_proxy_continuations/http_followups/memory_metadata.rs"]
mod http_memory_metadata;
#[path = "runtime_proxy_continuations/http_tool_and_compact.rs"]
mod http_tool_and_compact;
#[path = "runtime_proxy_continuations/http_compact_transparency.rs"]
mod http_compact_transparency;
#[path = "runtime_proxy_continuations/http_backend_passthrough.rs"]
mod http_backend_passthrough;
#[path = "runtime_proxy_continuations/post_commit.rs"]
mod post_commit;
