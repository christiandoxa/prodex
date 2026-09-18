use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketEventKind {
    pub precommit_hold: bool,
    pub realtime_terminal: bool,
    pub responses_terminal: bool,
}

pub fn websocket_event_kind(kind: &str) -> Result<WebsocketEventKind, MojoError> {
    ensure_rich_abi()?;
    if kind.len() > 4_096 {
        return Ok(WebsocketEventKind {
            precommit_hold: false,
            realtime_terminal: false,
            responses_terminal: false,
        });
    }
    let kind = view(kind);
    let mut output = [0_i64; 3];
    let status = unsafe {
        prodex_runtime_websocket_event_kind_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&kind),
            output.as_mut_ptr(),
        )
    };
    if status != 0 || output.iter().any(|value| !matches!(value, 0 | 1)) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(WebsocketEventKind {
        precommit_hold: output[0] == 1,
        realtime_terminal: output[1] == 1,
        responses_terminal: output[2] == 1,
    })
}
