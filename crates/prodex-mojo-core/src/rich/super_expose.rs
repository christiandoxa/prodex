use super::{ensure_rich_abi, mojo_mut_pointer_address};
use crate::MojoError;

const SUPER_EXPOSE_ABI_VERSION: i64 = 1;
const SUPER_EXPOSE_MAX_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeMethod {
    Unknown,
    ServerDiscover,
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
    Events,
    SessionPromptWrite,
    SessionPreempt,
    SessionOutputRead,
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
    fn prodex_mojo_super_expose_tunnel_id_valid_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_tunnel_client_version_valid_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
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

pub fn super_expose_tunnel_id_valid(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if value.len() > SUPER_EXPOSE_MAX_NAME_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_mojo_super_expose_tunnel_id_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
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

pub fn super_expose_tunnel_client_version_output_valid(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if value.len() > 16_384 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_mojo_super_expose_tunnel_client_version_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_client_version_policy_accepts_latest_and_compat_release() {
        assert!(
            super_expose_tunnel_client_version_output_valid(
                "0.0.14+0f870e50a973fa820d4c409000059e181e8d242b (git sha: 0f870e50a973fa820d4c409000059e181e8d242b)"
            )
            .unwrap()
        );
        assert!(
            super_expose_tunnel_client_version_output_valid(
                "0.0.13+4b5267f823be0b046bb883aacb51603cfde3a0ea (git sha: 4b5267f823be0b046bb883aacb51603cfde3a0ea)"
            )
            .unwrap()
        );
    }

    #[test]
    fn tunnel_client_version_policy_rejects_unvetted_builds() {
        assert!(
            !super_expose_tunnel_client_version_output_valid(
                "0.0.13+0000000000000000000000000000000000000000 (git sha: 0000000000000000000000000000000000000000)"
            )
            .unwrap()
        );
        assert!(!super_expose_tunnel_client_version_output_valid("0.0.12").unwrap());
    }
}
