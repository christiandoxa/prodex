use super::*;

#[path = "runtime_proxy_continuations/websocket_precommit.rs"]
mod websocket_precommit;
#[path = "runtime_proxy_continuations/http_followups.rs"]
mod http_followups;
#[path = "runtime_proxy_continuations/http_tool_and_compact.rs"]
mod http_tool_and_compact;
#[path = "runtime_proxy_continuations/http_backend_passthrough.rs"]
mod http_backend_passthrough;
#[path = "runtime_proxy_continuations/post_commit.rs"]
mod post_commit;
