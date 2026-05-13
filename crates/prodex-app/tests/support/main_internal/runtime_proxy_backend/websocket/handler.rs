use super::super::*;

#[path = "handler/accepted.rs"]
mod accepted;
#[path = "handler/dispatch.rs"]
mod dispatch;
#[path = "handler/entry.rs"]
mod entry;
#[path = "handler/events.rs"]
mod events;
#[path = "handler/main_account.rs"]
mod main_account;
#[path = "handler/realtime.rs"]
mod realtime;
#[path = "handler/request.rs"]
mod request;
#[path = "handler/second_account.rs"]
mod second_account;
#[path = "handler/third_account.rs"]
mod third_account;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BackendWebsocketAction {
    Continue,
    Break,
}

#[allow(clippy::result_large_err)]
pub(super) fn handle_runtime_proxy_backend_websocket(
    stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    websocket_requests: &Arc<Mutex<Vec<String>>>,
    mode: RuntimeProxyBackendMode,
) {
    entry::handle_runtime_proxy_backend_websocket(
        stream,
        responses_accounts,
        responses_headers,
        websocket_requests,
        mode,
    );
}
