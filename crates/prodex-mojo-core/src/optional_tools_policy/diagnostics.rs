use super::{ABI_VERSION, OptionalToolConfigShapeIssue, RequiredMcpConfigIssue};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_optional_tool_diagnostic_v1(
        version: i64,
        group: i64,
        issue: i64,
        name_address: u64,
        name_length: i64,
        output_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Render the exact diagnostic for one compiled selected-section shape issue.
pub fn optional_tool_config_shape_diagnostic(
    issue: OptionalToolConfigShapeIssue,
) -> Result<String, MojoError> {
    render(0, issue as i64, "")
}

/// Render the exact required-MCP diagnostic with the original server name.
pub fn optional_tool_required_mcp_diagnostic(
    issue: RequiredMcpConfigIssue,
    name: &str,
) -> Result<String, MojoError> {
    render(1, issue as i64, name)
}

fn render(group: i64, issue: i64, name: &str) -> Result<String, MojoError> {
    let capacity = name.len().checked_add(256).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_optional_tool_diagnostic_v1(
            ABI_VERSION,
            group,
            issue,
            name.as_ptr() as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            &mut written as *mut i64 as u64,
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
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
#[path = "diagnostics_tests.rs"]
mod tests;
