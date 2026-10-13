use super::{ensure_rich_abi, mojo_mut_pointer_address};
use crate::MojoError;

#[path = "super_expose/tunnel.rs"]
mod tunnel;
pub use tunnel::{super_expose_tunnel_client_version_output_valid, super_expose_tunnel_id_valid};

const SUPER_EXPOSE_ABI_VERSION: i64 = 1;
const SUPER_EXPOSE_MAX_NAME_BYTES: usize = 128;
const SUPER_EXPOSE_MAX_CONCURRENCY_PRESETS: usize = 32;
const SUPER_EXPOSE_OUTPUT_CURSOR_ABI_VERSION: i64 = 1;

#[cfg(test)]
#[path = "super_expose/tests.rs"]
mod concurrency_tests;

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeMethod {
    Unknown = 0,
    Initialize = 1,
    Ping = 2,
    ToolsList = 3,
    ToolsCall = 4,
    Notification = 5,
    ServerDiscover = 6,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeTool {
    Unknown = 0,
    Start = 1,
    Status = 2,
    Result = 3,
    Cancel = 4,
    List = 5,
    Exec = 6,
    Events = 7,
    SessionPromptWrite = 8,
    SessionPreempt = 9,
    SessionOutputRead = 10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuperExposeRoute {
    pub method: SuperExposeMethod,
    pub tool: SuperExposeTool,
}

unsafe extern "C" {
    fn prodex_mojo_super_expose_route_v1(
        abi_version: i64,
        method_address: u64,
        method_length: i64,
        tool_address: u64,
        tool_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_label_v1(
        abi_version: i64,
        label_kind: i64,
        value: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_tool_allowed_v1(
        abi_version: i64,
        mode: i64,
        tool_address: u64,
        tool_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_concurrency_plan_v1(
        abi_version: i64,
        default_value: i64,
        preset_address: u64,
        preset_count: i64,
        hard_max: i64,
        selected_index: i64,
        menu_address: u64,
        menu_capacity: i64,
        menu_count_address: u64,
        selected_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeConcurrencyChoice {
    Default,
    Preset(u16),
    Custom,
}

pub fn super_expose_route(method: &str, tool: Option<&str>) -> Result<SuperExposeRoute, MojoError> {
    ensure_rich_abi()?;
    let tool = tool.unwrap_or_default();
    if method.len() > SUPER_EXPOSE_MAX_NAME_BYTES || tool.len() > SUPER_EXPOSE_MAX_NAME_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; 2];
    let status = unsafe {
        prodex_mojo_super_expose_route_v1(
            SUPER_EXPOSE_ABI_VERSION,
            method.as_ptr() as u64,
            i64::try_from(method.len()).map_err(|_| MojoError::InvalidInput)?,
            tool.as_ptr() as u64,
            i64::try_from(tool.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    Ok(SuperExposeRoute {
        method: match output[0] {
            6 => SuperExposeMethod::ServerDiscover,
            1 => SuperExposeMethod::Initialize,
            2 => SuperExposeMethod::Ping,
            3 => SuperExposeMethod::ToolsList,
            4 => SuperExposeMethod::ToolsCall,
            5 => SuperExposeMethod::Notification,
            0 => SuperExposeMethod::Unknown,
            _ => return Err(MojoError::InvalidOutput),
        },
        tool: match output[1] {
            1 => SuperExposeTool::Start,
            2 => SuperExposeTool::Status,
            3 => SuperExposeTool::Result,
            4 => SuperExposeTool::Cancel,
            5 => SuperExposeTool::List,
            6 => SuperExposeTool::Exec,
            7 => SuperExposeTool::Events,
            8 => SuperExposeTool::SessionPromptWrite,
            9 => SuperExposeTool::SessionPreempt,
            10 => SuperExposeTool::SessionOutputRead,
            0 => SuperExposeTool::Unknown,
            _ => return Err(MojoError::InvalidOutput),
        },
    })
}

fn super_expose_label(label_kind: i64, value: i64) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let mut output = [0_u8; 32];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_super_expose_label_v1(
            SUPER_EXPOSE_ABI_VERSION,
            label_kind,
            value,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

impl SuperExposeMethod {
    pub fn label(self) -> Result<String, MojoError> {
        super_expose_label(0, self as i64)
    }
}

impl SuperExposeTool {
    pub fn label(self) -> Result<String, MojoError> {
        super_expose_label(1, self as i64)
    }
}

pub fn super_expose_tool_allowed(exec_only: bool, tool: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if tool.len() > SUPER_EXPOSE_MAX_NAME_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_mojo_super_expose_tool_allowed_v1(
            SUPER_EXPOSE_ABI_VERSION,
            i64::from(exec_only),
            tool.as_ptr() as u64,
            i64::try_from(tool.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn super_expose_concurrency_plan(
    default_value: u16,
    presets: &[u16],
    hard_max: u16,
    selected_index: Option<usize>,
) -> Result<
    (
        Vec<SuperExposeConcurrencyChoice>,
        Option<SuperExposeConcurrencyChoice>,
    ),
    MojoError,
> {
    ensure_rich_abi()?;
    if presets.len() > SUPER_EXPOSE_MAX_CONCURRENCY_PRESETS {
        return Err(MojoError::InvalidInput);
    }
    let choice_count = presets.len() + 2;
    let preset_values = presets
        .iter()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    let mut menu = vec![-1_i64; choice_count * 2];
    let mut menu_count = -1_i64;
    let mut selected = [-1_i64; 2];
    let selected_requested = selected_index.is_some();
    let selected_index = selected_index.map_or(Ok(-1), |index| {
        i64::try_from(index).map_err(|_| MojoError::InvalidInput)
    })?;
    let status = unsafe {
        prodex_mojo_super_expose_concurrency_plan_v1(
            SUPER_EXPOSE_ABI_VERSION,
            i64::from(default_value),
            preset_values.as_ptr() as usize as u64,
            i64::try_from(presets.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(hard_max),
            selected_index,
            mojo_mut_pointer_address(menu.as_mut_ptr()),
            i64::try_from(menu.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut menu_count),
            mojo_mut_pointer_address(selected.as_mut_ptr()),
        )
    };
    super_expose_status(status)?;
    if menu_count != choice_count as i64 {
        return Err(MojoError::InvalidOutput);
    }
    let decode = |kind, value| match kind {
        0 if value == i64::from(default_value) => Ok(SuperExposeConcurrencyChoice::Default),
        1 if u16::try_from(value)
            .is_ok_and(|value| value <= hard_max && presets.contains(&value)) =>
        {
            Ok(SuperExposeConcurrencyChoice::Preset(
                u16::try_from(value).map_err(|_| MojoError::InvalidOutput)?,
            ))
        }
        2 if value == 0 => Ok(SuperExposeConcurrencyChoice::Custom),
        _ => Err(MojoError::InvalidOutput),
    };
    let choices = menu
        .as_chunks::<2>()
        .0
        .iter()
        .map(|choice| decode(choice[0], choice[1]))
        .collect::<Result<Vec<_>, _>>()?;
    let selected = selected_requested
        .then(|| decode(selected[0], selected[1]))
        .transpose()?;
    Ok((choices, selected))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeProtocolDecision {
    Ok,
    UnsupportedVersion,
    VersionMismatch,
    MetadataRequired,
    MethodMismatch,
    NameMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeDispatchDecision {
    Ok,
    NotificationAccepted,
    NotificationUnsupported,
    InvalidRequestId,
    InitializeParamsRequired,
    ProtocolVersionRequired,
    ToolParamsRequired,
    ToolNameRequired,
    ToolArgumentsObjectRequired,
}

unsafe extern "C" {
    fn prodex_mojo_super_expose_dispatch_validation_v1(
        abi_version: i64,
        method_address: u64,
        method_length: i64,
        has_id_field: i64,
        id_valid: i64,
        params_is_object: i64,
        protocol_version_present: i64,
        tool_name_present: i64,
        tool_arguments_kind: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_protocol_version_supported_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_protocol_metadata_v1(
        abi_version: i64,
        method_address: u64,
        method_length: i64,
        header_present: i64,
        header_address: u64,
        header_length: i64,
        body_present: i64,
        body_address: u64,
        body_length: i64,
        method_header_present: i64,
        method_header_address: u64,
        method_header_length: i64,
        name_header_present: i64,
        name_header_address: u64,
        name_header_length: i64,
        body_name_present: i64,
        body_name_address: u64,
        body_name_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_media_header_v1(
        abi_version: i64,
        kind: i64,
        present: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_json_nesting_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        limit: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_tool_argument_allowed_v1(
        abi_version: i64,
        tool_address: u64,
        tool_length: i64,
        key_address: u64,
        key_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_run_id_valid_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_string_valid_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        max_bytes: i64,
        output_address: u64,
    ) -> i64;
}

fn super_expose_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn optional_string_parts(value: Option<&str>) -> Result<(i64, u64, i64), MojoError> {
    match value {
        Some(value) => Ok((
            1,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        )),
        None => Ok((0, 0, 0)),
    }
}

fn bool_output(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn super_expose_dispatch_validation(
    method: &str,
    has_id_field: bool,
    id_valid: bool,
    params_is_object: bool,
    protocol_version_present: bool,
    tool_name_present: bool,
    tool_arguments_kind: i64,
) -> Result<SuperExposeDispatchDecision, MojoError> {
    ensure_rich_abi()?;
    if method.len() > SUPER_EXPOSE_MAX_NAME_BYTES || !(0..=2).contains(&tool_arguments_kind) {
        return Err(MojoError::InvalidInput);
    }
    let mut output = -1_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_dispatch_validation_v1(
            SUPER_EXPOSE_ABI_VERSION,
            method.as_ptr() as usize as u64,
            i64::try_from(method.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(has_id_field),
            i64::from(id_valid),
            i64::from(params_is_object),
            i64::from(protocol_version_present),
            i64::from(tool_name_present),
            tool_arguments_kind,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    match output {
        0 => Ok(SuperExposeDispatchDecision::Ok),
        1 => Ok(SuperExposeDispatchDecision::NotificationAccepted),
        2 => Ok(SuperExposeDispatchDecision::NotificationUnsupported),
        3 => Ok(SuperExposeDispatchDecision::InvalidRequestId),
        4 => Ok(SuperExposeDispatchDecision::InitializeParamsRequired),
        5 => Ok(SuperExposeDispatchDecision::ProtocolVersionRequired),
        6 => Ok(SuperExposeDispatchDecision::ToolParamsRequired),
        7 => Ok(SuperExposeDispatchDecision::ToolNameRequired),
        8 => Ok(SuperExposeDispatchDecision::ToolArgumentsObjectRequired),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn super_expose_protocol_version_supported(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_protocol_version_supported_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_protocol_metadata(
    method: &str,
    header_version: Option<&str>,
    body_version: Option<&str>,
    method_header: Option<&str>,
    name_header: Option<&str>,
    body_name: Option<&str>,
) -> Result<SuperExposeProtocolDecision, MojoError> {
    ensure_rich_abi()?;
    let header = optional_string_parts(header_version)?;
    let body = optional_string_parts(body_version)?;
    let method_header = optional_string_parts(method_header)?;
    let name_header = optional_string_parts(name_header)?;
    let body_name = optional_string_parts(body_name)?;
    let mut output = -1_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_protocol_metadata_v1(
            SUPER_EXPOSE_ABI_VERSION,
            method.as_ptr() as usize as u64,
            i64::try_from(method.len()).map_err(|_| MojoError::InvalidInput)?,
            header.0,
            header.1,
            header.2,
            body.0,
            body.1,
            body.2,
            method_header.0,
            method_header.1,
            method_header.2,
            name_header.0,
            name_header.1,
            name_header.2,
            body_name.0,
            body_name.1,
            body_name.2,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    match output {
        0 => Ok(SuperExposeProtocolDecision::Ok),
        1 => Ok(SuperExposeProtocolDecision::UnsupportedVersion),
        2 => Ok(SuperExposeProtocolDecision::VersionMismatch),
        3 => Ok(SuperExposeProtocolDecision::MetadataRequired),
        4 => Ok(SuperExposeProtocolDecision::MethodMismatch),
        5 => Ok(SuperExposeProtocolDecision::NameMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn media_header_allowed(kind: i64, value: Option<&str>) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let value = optional_string_parts(value)?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_media_header_v1(
            SUPER_EXPOSE_ABI_VERSION,
            kind,
            value.0,
            value.1,
            value.2,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_content_type_allowed(value: Option<&str>) -> Result<bool, MojoError> {
    media_header_allowed(0, value)
}

pub fn super_expose_accept_allowed(value: Option<&str>) -> Result<bool, MojoError> {
    media_header_allowed(1, value)
}

pub fn super_expose_json_nesting_within_limit(
    body: &[u8],
    limit: usize,
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_json_nesting_v1(
            SUPER_EXPOSE_ABI_VERSION,
            body.as_ptr() as usize as u64,
            i64::try_from(body.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(limit).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_tool_argument_allowed(tool: &str, key: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_tool_argument_allowed_v1(
            SUPER_EXPOSE_ABI_VERSION,
            tool.as_ptr() as usize as u64,
            i64::try_from(tool.len()).map_err(|_| MojoError::InvalidInput)?,
            key.as_ptr() as usize as u64,
            i64::try_from(key.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_run_id_valid(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_run_id_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_string_valid(value: &str, max_bytes: usize) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_string_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

/// Facts used to validate one opaque session-output cursor after decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuperExposeOutputCursorInput {
    pub encoded_length: usize,
    pub version: u8,
    pub prodex_pid: u32,
    pub codex_pid: u32,
    pub prodex_birth_present: bool,
    pub codex_birth_present: bool,
    pub thread_id_present: bool,
    pub source_id_present: bool,
    pub checkpoint_id_present: bool,
    pub event_index: usize,
}

unsafe extern "C" {
    fn prodex_mojo_super_expose_output_cursor_valid_v1(
        abi_version: i64,
        encoded_length: i64,
        version: i64,
        prodex_pid: i64,
        codex_pid: i64,
        prodex_birth_present: i64,
        codex_birth_present: i64,
        thread_id_present: i64,
        source_id_present: i64,
        checkpoint_id_present: i64,
        event_index: u64,
        output_address: u64,
    ) -> i64;
}

/// Validate decoded cursor identity, version, required fields, and event bound in Mojo.
pub fn super_expose_output_cursor_valid(
    input: SuperExposeOutputCursorInput,
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_super_expose_output_cursor_valid_v1(
            SUPER_EXPOSE_OUTPUT_CURSOR_ABI_VERSION,
            i64::try_from(input.encoded_length).map_err(|_| MojoError::InvalidInput)?,
            i64::from(input.version),
            i64::from(input.prodex_pid),
            i64::from(input.codex_pid),
            i64::from(input.prodex_birth_present),
            i64::from(input.codex_birth_present),
            i64::from(input.thread_id_present),
            i64::from(input.source_id_present),
            i64::from(input.checkpoint_id_present),
            u64::try_from(input.event_index).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    };
    super_expose_status(status)?;
    bool_output(output)
}

/// Apply the encoded cursor length bound before the host decodes untrusted input.
pub fn super_expose_output_cursor_length_valid(encoded_length: usize) -> Result<bool, MojoError> {
    super_expose_output_cursor_valid(SuperExposeOutputCursorInput {
        encoded_length,
        version: 1,
        prodex_pid: 1,
        codex_pid: 1,
        prodex_birth_present: true,
        codex_birth_present: true,
        thread_id_present: true,
        source_id_present: true,
        checkpoint_id_present: true,
        event_index: 0,
    })
}

#[cfg(test)]
#[path = "super_expose/protocol_tests.rs"]
mod protocol_policy_tests;
