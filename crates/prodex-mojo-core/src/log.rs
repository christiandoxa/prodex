use crate::MojoError;

const LOG_LEVEL_ABI_VERSION: i64 = 1;
const LOG_LEVEL_MAX_BYTES: usize = 4096;

const _: () = assert!(std::mem::size_of::<usize>() == std::mem::size_of::<u64>());

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct LogStringView {
    ptr: u64,
    len: u64,
}

unsafe extern "C" {
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
    classify_log_level("level=error") == Ok(Some("error"))
        && classify_log_level("2026-05-05T00:00:00Z info heartbeat") == Ok(Some("info"))
}

const TRANSCRIPT_ABI_VERSION: i64 = 1;
const TRANSCRIPT_TOOL_NAME_MAX_BYTES: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptEventKind {
    Unknown,
    Protocol,
    StatusTerminal,
    StatusError,
    User,
    Assistant,
    Reasoning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptProtocolSource {
    Tool,
    Mcp,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptEventPlan {
    pub kind: TranscriptEventKind,
    pub protocol_source: TranscriptProtocolSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptItemKind {
    Unknown,
    Message,
    FunctionCall,
    FunctionOutput,
    CustomCall,
    CustomOutput,
    ShellCall,
    ShellOutput,
    Reasoning,
    Protocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscriptItemPlan {
    pub kind: TranscriptItemKind,
    pub protocol_source: TranscriptProtocolSource,
}

unsafe extern "C" {
    fn prodex_mojo_transcript_event_classify_v1(
        abi_version: i64,
        event_address: u64,
        event_length: i64,
        status_address: u64,
        status_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_transcript_item_classify_v1(
        abi_version: i64,
        item_address: u64,
        item_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_transcript_operation_span_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_transcript_tool_name_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn transcript_input_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn transcript_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn transcript_protocol_source(value: i64) -> Result<TranscriptProtocolSource, MojoError> {
    match value {
        0 => Ok(TranscriptProtocolSource::Tool),
        1 => Ok(TranscriptProtocolSource::Mcp),
        2 => Ok(TranscriptProtocolSource::Agent),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn classify_transcript_event(
    event_type: &str,
    status: Option<&str>,
) -> Result<TranscriptEventPlan, MojoError> {
    let status_value = status.unwrap_or_default();
    let mut output = [0_i64; 2];
    transcript_status(unsafe {
        prodex_mojo_transcript_event_classify_v1(
            TRANSCRIPT_ABI_VERSION,
            event_type.as_ptr() as usize as u64,
            transcript_input_len(event_type)?,
            status_value.as_ptr() as usize as u64,
            transcript_input_len(status_value)?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    let kind = match output[0] {
        0 => TranscriptEventKind::Unknown,
        1 => TranscriptEventKind::Protocol,
        2 => TranscriptEventKind::StatusTerminal,
        3 => TranscriptEventKind::StatusError,
        4 => TranscriptEventKind::User,
        5 => TranscriptEventKind::Assistant,
        6 => TranscriptEventKind::Reasoning,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(TranscriptEventPlan {
        kind,
        protocol_source: transcript_protocol_source(output[1])?,
    })
}

pub fn classify_transcript_item(item_type: &str) -> Result<TranscriptItemPlan, MojoError> {
    let mut output = [0_i64; 2];
    transcript_status(unsafe {
        prodex_mojo_transcript_item_classify_v1(
            TRANSCRIPT_ABI_VERSION,
            item_type.as_ptr() as usize as u64,
            transcript_input_len(item_type)?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    let kind = match output[0] {
        0 => TranscriptItemKind::Unknown,
        1 => TranscriptItemKind::Message,
        2 => TranscriptItemKind::FunctionCall,
        3 => TranscriptItemKind::FunctionOutput,
        4 => TranscriptItemKind::CustomCall,
        5 => TranscriptItemKind::CustomOutput,
        6 => TranscriptItemKind::ShellCall,
        7 => TranscriptItemKind::ShellOutput,
        8 => TranscriptItemKind::Reasoning,
        9 => TranscriptItemKind::Protocol,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(TranscriptItemPlan {
        kind,
        protocol_source: transcript_protocol_source(output[1])?,
    })
}

pub fn transcript_operation_span(value: &str) -> Result<Option<(&str, bool)>, MojoError> {
    let mut output = [0_i64; 2];
    transcript_status(unsafe {
        prodex_mojo_transcript_operation_span_v1(
            TRANSCRIPT_ABI_VERSION,
            value.as_ptr() as usize as u64,
            transcript_input_len(value)?,
            mutable_pointer_address(output.as_mut_ptr()),
        )
    })?;
    if output[0] == 0 {
        return Ok(None);
    }
    let end = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let truncated = match output[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    value
        .get(..end)
        .map(|value| Some((value, truncated)))
        .ok_or(MojoError::InvalidOutput)
}

pub fn sanitize_transcript_tool_name(value: &str) -> Result<String, MojoError> {
    let mut output = [0_u8; TRANSCRIPT_TOOL_NAME_MAX_BYTES];
    let mut written = 0_i64;
    transcript_status(unsafe {
        prodex_mojo_transcript_tool_name_v1(
            TRANSCRIPT_ABI_VERSION,
            value.as_ptr() as usize as u64,
            transcript_input_len(value)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mutable_pointer_address(&mut written),
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod transcript_policy_tests {
    use super::*;

    #[test]
    fn transcript_event_and_item_classifiers_preserve_rust_precedence() {
        assert_eq!(
            classify_transcript_event("mcp_tool_call", Some("failed")).unwrap(),
            TranscriptEventPlan {
                kind: TranscriptEventKind::Protocol,
                protocol_source: TranscriptProtocolSource::Mcp,
            }
        );
        assert_eq!(
            classify_transcript_event("turn_failed", None).unwrap().kind,
            TranscriptEventKind::StatusError
        );
        assert_eq!(
            classify_transcript_event("command_status_update", Some("ok"))
                .unwrap()
                .kind,
            TranscriptEventKind::StatusTerminal
        );
        assert_eq!(
            classify_transcript_item("vendor_tool_call").unwrap().kind,
            TranscriptItemKind::Unknown
        );
        assert_eq!(
            classify_transcript_item("sub_agent_tool_call").unwrap(),
            TranscriptItemPlan {
                kind: TranscriptItemKind::Protocol,
                protocol_source: TranscriptProtocolSource::Agent,
            }
        );
        assert_eq!(
            classify_transcript_item("computer_call").unwrap(),
            TranscriptItemPlan {
                kind: TranscriptItemKind::Protocol,
                protocol_source: TranscriptProtocolSource::Tool,
            }
        );
    }

    #[test]
    fn transcript_operation_value_policy_preserves_whitespace_and_unicode_bounds() {
        assert_eq!(
            transcript_operation_span("  visible  ").unwrap(),
            Some(("  visible  ", false))
        );
        assert_eq!(transcript_operation_span(" 　	 ").unwrap(), None);
        assert_eq!(transcript_operation_span("badvalue").unwrap(), None);

        let value = format!("{}é-tail", "a".repeat(191));
        let (bounded, truncated) = transcript_operation_span(&value).unwrap().unwrap();
        assert_eq!(bounded, format!("{}é", "a".repeat(191)));
        assert!(truncated);
    }

    #[test]
    fn transcript_tool_name_policy_is_ascii_safe_and_character_bounded() {
        assert_eq!(sanitize_transcript_tool_name("").unwrap(), "tool");
        assert_eq!(
            sanitize_transcript_tool_name("mcp.模型:call").unwrap(),
            "mcp.__:call"
        );
        assert_eq!(
            sanitize_transcript_tool_name(&"x".repeat(120)).unwrap(),
            "x".repeat(96)
        );
    }
}
