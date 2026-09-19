use super::{ensure_rich_abi, mojo_mut_pointer_address};
use crate::MojoError;

const SUPER_EXPOSE_ABI_VERSION: i64 = 1;
const SUPER_EXPOSE_MAX_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeMethod {
    Unknown,
    Initialize,
    Ping,
    ToolsList,
    ToolsCall,
    Notification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeTool {
    Unknown,
    Start,
    Status,
    Result,
    Cancel,
    List,
    Exec,
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
    fn prodex_mojo_super_expose_tool_allowed_v1(
        abi_version: i64,
        mode: i64,
        tool_address: u64,
        tool_length: i64,
        output_address: u64,
    ) -> i64;
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
            0 => SuperExposeTool::Unknown,
            _ => return Err(MojoError::InvalidOutput),
        },
    })
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
