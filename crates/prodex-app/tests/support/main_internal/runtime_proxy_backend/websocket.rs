use super::*;

#[path = "websocket/handler.rs"]
mod handler;
#[path = "websocket/tool.rs"]
mod tool;

pub(crate) fn runtime_proxy_backend_is_websocket_upgrade(stream: &TcpStream) -> bool {
    let mut buffer = [0_u8; 2048];
    let Ok(read) = stream.peek(&mut buffer) else {
        return false;
    };
    if read == 0 {
        return false;
    }
    let request = String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase();
    request.contains("upgrade: websocket")
}

pub(crate) fn handle_runtime_proxy_backend_websocket(
    stream: TcpStream,
    responses_accounts: &Arc<Mutex<Vec<String>>>,
    responses_headers: &Arc<Mutex<Vec<BTreeMap<String, String>>>>,
    websocket_requests: &Arc<Mutex<Vec<String>>>,
    mode: RuntimeProxyBackendMode,
) {
    handler::handle_runtime_proxy_backend_websocket(
        stream,
        responses_accounts,
        responses_headers,
        websocket_requests,
        mode,
    );
}
