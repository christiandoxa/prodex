use crate::MojoError;

const LOG_ABI_VERSION: i64 = 4;
const LOG_LEVEL_ABI_VERSION: i64 = 1;
const LOG_LEVEL_MAX_BYTES: usize = 4096;

const _: () = assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct LogStringView {
    ptr: u64,
    len: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogCategory {
    None,
    Route,
    Quota,
    Backoff,
    Http,
    Websocket,
    Stream,
    Smart,
    Compact,
    Error,
    Hook,
    Request,
    Model,
    Mcp,
    Agent,
    Tool,
    Retry,
    Health,
    Upstream,
    Response,
    Terminal,
    Load,
}

impl LogCategory {
    pub fn source(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Route => Some("route"),
            Self::Quota => Some("quota"),
            Self::Backoff => Some("backoff"),
            Self::Http => Some("upstream"),
            Self::Websocket => Some("ws"),
            Self::Stream => Some("stream"),
            Self::Smart => Some("smart"),
            Self::Compact => Some("compact"),
            Self::Error => Some("error"),
            Self::Hook => Some("hook"),
            Self::Request => Some("request"),
            Self::Model => Some("model"),
            Self::Mcp => Some("mcp"),
            Self::Agent => Some("agent"),
            Self::Tool => Some("tool"),
            Self::Retry => Some("retry"),
            Self::Health => Some("health"),
            Self::Upstream => Some("upstream"),
            Self::Response => Some("response"),
            Self::Terminal => Some("terminal"),
            Self::Load => Some("load"),
        }
    }
}

unsafe extern "C" {
    fn prodex_mojo_log_classify_v3(
        abi_version: i64,
        event: u64,
        category: u64,
        severity: u64,
    ) -> i64;
    fn prodex_mojo_log_level_classify_v1(abi_version: i64, event: u64, level: u64) -> i64;
}

#[inline]
fn pointer_address<T>(pointer: *const T) -> u64 {
    pointer as usize as u64
}

#[inline]
fn mutable_pointer_address<T>(pointer: *mut T) -> u64 {
    pointer as usize as u64
}

pub fn classify_log_event(event: &str) -> Result<(LogCategory, i64), MojoError> {
    let mut category = -1_i64;
    let mut severity = -1_i64;
    let event_view = LogStringView {
        ptr: event.as_ptr() as usize as u64,
        len: event.len() as u64,
    };
    let status = unsafe {
        prodex_mojo_log_classify_v3(
            LOG_ABI_VERSION,
            pointer_address(&event_view),
            mutable_pointer_address(&mut category),
            mutable_pointer_address(&mut severity),
        )
    };
    if status == 2 {
        return Err(MojoError::InvalidInput);
    }
    if status != 0 {
        return Err(MojoError::AbiMismatch);
    }
    let category = match category {
        0 => LogCategory::None,
        1 => LogCategory::Route,
        2 => LogCategory::Quota,
        3 => LogCategory::Backoff,
        4 => LogCategory::Http,
        5 => LogCategory::Websocket,
        6 => LogCategory::Stream,
        7 => LogCategory::Smart,
        8 => LogCategory::Compact,
        9 => LogCategory::Error,
        10 => LogCategory::Hook,
        11 => LogCategory::Request,
        12 => LogCategory::Model,
        13 => LogCategory::Mcp,
        14 => LogCategory::Agent,
        15 => LogCategory::Tool,
        16 => LogCategory::Retry,
        17 => LogCategory::Health,
        18 => LogCategory::Upstream,
        19 => LogCategory::Response,
        20 => LogCategory::Terminal,
        21 => LogCategory::Load,
        _ => return Err(MojoError::InvalidOutput),
    };
    (0..=3)
        .contains(&severity)
        .then_some((category, severity))
        .ok_or(MojoError::InvalidOutput)
}

/// Classifies an already-normalized, length-bounded log line by its level.
pub fn classify_log_level(line: &str) -> Result<Option<&'static str>, MojoError> {
    if line.len() > LOG_LEVEL_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut level = -1_i64;
    let line_view = LogStringView {
        ptr: line.as_ptr() as usize as u64,
        len: line.len() as u64,
    };
    let status = unsafe {
        prodex_mojo_log_level_classify_v1(
            LOG_LEVEL_ABI_VERSION,
            pointer_address(&line_view),
            mutable_pointer_address(&mut level),
        )
    };
    if status == 2 {
        return Err(MojoError::InvalidInput);
    }
    if status != 0 {
        return Err(MojoError::AbiMismatch);
    }
    match level {
        0 => Ok(None),
        1 => Ok(Some("fatal")),
        2 => Ok(Some("error")),
        3 => Ok(Some("warn")),
        4 => Ok(Some("info")),
        5 => Ok(Some("debug")),
        6 => Ok(Some("trace")),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn self_test() -> bool {
    classify_log_event("selection_pick")
        .is_ok_and(|(category, severity)| category == LogCategory::Route && severity == 1)
        && classify_log_event("upstream_start")
            .is_ok_and(|(category, severity)| category == LogCategory::Http && severity == 1)
        && classify_log_event("context_compacted")
            .is_ok_and(|(category, severity)| category == LogCategory::Compact && severity == 1)
        && classify_log_level("level=error") == Ok(Some("error"))
        && classify_log_level("2026-05-05T00:00:00Z info heartbeat") == Ok(Some("info"))
}
