use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_SERVERS: usize = 65_536;

/// Facts extracted by Rust from one Kiro MCP server entry.
///
/// The record deliberately excludes command paths, environment values, and other
/// document text. Mojo owns only the bounded compatibility action for these facts.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct KiroMcpServerFacts {
    pub server_name_matches: i64,
    pub command_name_matches: i64,
    pub disabled_tools_state: KiroMcpDisabledToolsState,
    pub incompatible_tool_present: i64,
}

const _: () = assert!(std::mem::size_of::<KiroMcpServerFacts>() == 32);

/// Shape of the `disabledTools` member before the compatibility action is applied.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KiroMcpDisabledToolsState {
    #[default]
    Missing = 0,
    Null = 1,
    Array = 2,
    Invalid = 3,
}

/// Compatibility action for one MCP server entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroMcpNormalizationAction {
    Keep,
    InitializeDisabledTools,
    AppendIncompatibleTool,
    InvalidDisabledTools,
}

unsafe extern "C" {
    fn prodex_kiro_mcp_normalization_plan_v1(
        abi_version: i64,
        facts_address: u64,
        fact_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Plan Kiro MCP compatibility edits in the compiled Mojo kernel.
pub fn plan_mcp_normalization(
    facts: &[KiroMcpServerFacts],
) -> Result<Vec<KiroMcpNormalizationAction>, MojoError> {
    if facts.len() > MAX_SERVERS {
        return Err(MojoError::InvalidInput);
    }
    let mut output = vec![0_i64; facts.len().max(1)];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_kiro_mcp_normalization_plan_v1(
            ABI_VERSION,
            facts.as_ptr() as usize as u64,
            i64::try_from(facts.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(facts.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
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
    if written != facts.len() {
        return Err(MojoError::InvalidOutput);
    }
    output[..written]
        .iter()
        .map(|action| match action {
            0 => Ok(KiroMcpNormalizationAction::Keep),
            1 => Ok(KiroMcpNormalizationAction::InitializeDisabledTools),
            2 => Ok(KiroMcpNormalizationAction::AppendIncompatibleTool),
            3 => Ok(KiroMcpNormalizationAction::InvalidDisabledTools),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_normalization_abi_rejects_bad_version_and_pointers() {
        let mut written = -1_i64;
        let status = unsafe {
            prodex_kiro_mcp_normalization_plan_v1(
                ABI_VERSION + 1,
                0,
                0,
                0,
                0,
                (&mut written as *mut i64) as usize as u64,
            )
        };
        assert_eq!(status, 4);
        let status = unsafe { prodex_kiro_mcp_normalization_plan_v1(ABI_VERSION, 0, 1, 1, 1, 0) };
        assert_eq!(status, 1);
    }

    #[test]
    fn mcp_normalization_abi_rejects_undersized_output_before_writing() {
        let facts = [KiroMcpServerFacts::default()];
        let mut output = [9_i64; 1];
        let mut written = -1_i64;
        let status = unsafe {
            prodex_kiro_mcp_normalization_plan_v1(
                ABI_VERSION,
                facts.as_ptr() as usize as u64,
                1,
                output.as_mut_ptr() as usize as u64,
                0,
                (&mut written as *mut i64) as usize as u64,
            )
        };
        assert_eq!(status, 3);
        assert_eq!(output, [9]);
        assert_eq!(written, -1);
    }
}
