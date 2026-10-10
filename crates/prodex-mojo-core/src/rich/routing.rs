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

/// Normalizes a runtime proxy mount path using Rust-compatible Unicode trim
/// and slash-boundary semantics. Rust materializes the validated bytes while
/// Mojo owns the deterministic path decision.
pub fn runtime_proxy_mount_path(mount_path: &str) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let capacity = mount_path
        .len()
        .checked_add(1)
        .ok_or(MojoError::InvalidInput)?
        .max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_proxy_mount_path_v1(
            1,
            mount_path.as_ptr() as usize as u64,
            i64::try_from(mount_path.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            &mut written,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    output
        .get(..written)
        .ok_or(MojoError::InvalidOutput)
        .and_then(|value| String::from_utf8(value.to_vec()).map_err(|_| MojoError::InvalidOutput))
}

fn runtime_proxy_bool_output(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn runtime_proxy_query_mark(path_and_query: &str, value: i64) -> Result<Option<usize>, MojoError> {
    if value < 0 {
        return Ok(None);
    }
    let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
    if index >= path_and_query.len()
        || path_and_query.as_bytes().get(index) != Some(&b'?')
        || !path_and_query.is_char_boundary(index)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(index))
}

fn runtime_proxy_mount_suffix_start(
    path_and_query: &str,
    path_end: usize,
    mounted: bool,
    value: i64,
) -> Result<Option<usize>, MojoError> {
    if !mounted {
        return (value == -1)
            .then_some(None)
            .ok_or(MojoError::InvalidOutput);
    }
    let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
    if index > path_end || !path_and_query.is_char_boundary(index) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(index))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeProviderRoutePlan {
    /// -1 unsupported; 0 responses; 1 compact; 2 chat; 3 messages;
    /// 4 embeddings; 5 models list; 6 models single.
    pub route_kind: i64,
    pub model_id_range: Option<(usize, usize)>,
    pub path_end: usize,
}

pub fn runtime_provider_route_plan(
    path_and_query: &str,
) -> Result<RuntimeProviderRoutePlan, MojoError> {
    ensure_rich_abi()?;
    let path_length = i64::try_from(path_and_query.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_runtime_provider_route_plan_v1(
            1,
            path_and_query.as_ptr() as usize as u64,
            path_length,
            output.as_mut_ptr(),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            _ => MojoError::InvalidOutput,
        });
    }
    if !(-1..=6).contains(&output[0]) {
        return Err(MojoError::InvalidOutput);
    }
    let path_end = usize::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?;
    if path_end > path_and_query.len() || !path_and_query.is_char_boundary(path_end) {
        return Err(MojoError::InvalidOutput);
    }
    let model_id_range = if output[0] == 6 {
        let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
        let end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
        if start >= end
            || end != path_end
            || !path_and_query.is_char_boundary(start)
            || !path_and_query.is_char_boundary(end)
        {
            return Err(MojoError::InvalidOutput);
        }
        Some((start, end))
    } else {
        if output[1] != -1 || output[2] != -1 {
            return Err(MojoError::InvalidOutput);
        }
        None
    };
    Ok(RuntimeProviderRoutePlan {
        route_kind: output[0],
        model_id_range,
        path_end,
    })
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
    let path_end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if path_end > path_and_query.len() || !path_and_query.is_char_boundary(path_end) {
        return Err(MojoError::InvalidOutput);
    }
    let query_mark = runtime_proxy_query_mark(path_and_query, output[3])?;
    let mounted = runtime_proxy_bool_output(output[0])?;
    let mount_suffix_start =
        runtime_proxy_mount_suffix_start(path_and_query, path_end, mounted, output[1])?;
    if !(0..=3).contains(&output[9]) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RuntimeProxyPathPlan {
        mount_suffix_start,
        path_end,
        query_mark,
        responses: runtime_proxy_bool_output(output[4])?,
        chat_completions: runtime_proxy_bool_output(output[5])?,
        compact: runtime_proxy_bool_output(output[6])?,
        realtime_call: runtime_proxy_bool_output(output[7])?,
        realtime_websocket: runtime_proxy_bool_output(output[8])?,
        route_kind: output[9],
        long_lived: runtime_proxy_bool_output(output[10])?,
    })
}
