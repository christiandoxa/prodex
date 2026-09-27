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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProxyPathPlan {
    pub mount_suffix_start: Option<usize>,
    pub path_end: usize,
    pub query_mark: Option<usize>,
    pub responses: bool,
    pub chat_completions: bool,
    pub compact: bool,
    pub realtime_call: bool,
    pub realtime_websocket: bool,
    pub route_kind: i64,
    pub long_lived: bool,
}

pub fn runtime_proxy_path_plan(
    path_and_query: &str,
    websocket: bool,
) -> Result<RuntimeProxyPathPlan, MojoError> {
    ensure_rich_abi()?;
    let path_length = i64::try_from(path_and_query.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [-1_i64; 11];
    let status = unsafe {
        prodex_runtime_proxy_path_plan_v1(
            1,
            path_and_query.as_ptr() as usize as u64,
            path_length,
            i64::from(websocket),
            output.as_mut_ptr(),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            _ => MojoError::InvalidOutput,
        });
    }
    let bool_output = |value: i64| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let path_end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if path_end > path_and_query.len() || !path_and_query.is_char_boundary(path_end) {
        return Err(MojoError::InvalidOutput);
    }
    let query_mark = if output[3] < 0 {
        None
    } else {
        let index = usize::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?;
        if index >= path_and_query.len()
            || path_and_query.as_bytes().get(index) != Some(&b'?')
            || !path_and_query.is_char_boundary(index)
        {
            return Err(MojoError::InvalidOutput);
        }
        Some(index)
    };
    let mounted = bool_output(output[0])?;
    let mount_suffix_start = if mounted {
        let index = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
        if index > path_end || !path_and_query.is_char_boundary(index) {
            return Err(MojoError::InvalidOutput);
        }
        Some(index)
    } else {
        if output[1] != -1 {
            return Err(MojoError::InvalidOutput);
        }
        None
    };
    if !(0..=3).contains(&output[9]) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RuntimeProxyPathPlan {
        mount_suffix_start,
        path_end,
        query_mark,
        responses: bool_output(output[4])?,
        chat_completions: bool_output(output[5])?,
        compact: bool_output(output[6])?,
        realtime_call: bool_output(output[7])?,
        realtime_websocket: bool_output(output[8])?,
        route_kind: output[9],
        long_lived: bool_output(output[10])?,
    })
}
