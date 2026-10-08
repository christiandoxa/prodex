use crate::MojoError;

const ABI_VERSION: i64 = 1;

pub const CAP_CODEX: u64 = 1;
pub const CAP_CLAUDE: u64 = 2;
pub const CAP_SHELL_COMPRESSION: u64 = 4;
pub const CAP_STRUCTURAL_NAVIGATION: u64 = 8;
pub const CAP_BROWSER_AUTOMATION: u64 = 16;
pub const CAP_SIMPLICITY_REVIEW: u64 = 32;
pub const CAP_REDACTION: u64 = 64;

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalToolPolicyId {
    Caveman = 0,
    Rtk = 1,
    CodebaseMemoryMcp = 2,
    PlaywrightMcp = 3,
    Ponytail = 4,
    Presidio = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalToolPolicyKind {
    Command,
    CodexPlugin,
    McpServer,
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionalToolDescriptorPolicy {
    pub kind: OptionalToolPolicyKind,
    pub capability_mask: u64,
    pub super_default: bool,
}

unsafe extern "C" {
    fn prodex_optional_tool_class_v1(abi_version: i64, address: u64, length: i64) -> i64;
    fn prodex_optional_tool_descriptor_policy_v1(abi_version: i64, tool_id: i64) -> i64;
    fn prodex_optional_tool_manifest_tree_supported_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        vetted_address: u64,
        vetted_length: i64,
        legacy_address: u64,
        legacy_length: i64,
    ) -> i64;
    fn prodex_optional_tool_hex_encode_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

pub fn optional_tool_class(value: &str) -> Result<Option<OptionalToolPolicyId>, MojoError> {
    let result = unsafe {
        prodex_optional_tool_class_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(OptionalToolPolicyId::Caveman)),
        1 => Ok(Some(OptionalToolPolicyId::Rtk)),
        2 => Ok(Some(OptionalToolPolicyId::CodebaseMemoryMcp)),
        3 => Ok(Some(OptionalToolPolicyId::PlaywrightMcp)),
        4 => Ok(Some(OptionalToolPolicyId::Ponytail)),
        5 => Ok(Some(OptionalToolPolicyId::Presidio)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn optional_tool_descriptor_policy(
    id: OptionalToolPolicyId,
) -> Result<OptionalToolDescriptorPolicy, MojoError> {
    let result = unsafe { prodex_optional_tool_descriptor_policy_v1(ABI_VERSION, id as i64) };
    if result == -2 {
        return Err(MojoError::InvalidInput);
    }
    if result < 0 {
        return Err(MojoError::InvalidOutput);
    }
    let kind = match result & 0xff {
        0 => OptionalToolPolicyKind::Command,
        1 => OptionalToolPolicyKind::CodexPlugin,
        2 => OptionalToolPolicyKind::McpServer,
        3 => OptionalToolPolicyKind::Service,
        _ => return Err(MojoError::InvalidOutput),
    };
    let capability_mask =
        u64::try_from((result >> 8) & 0xff).map_err(|_| MojoError::InvalidOutput)?;
    let super_default = ((result >> 16) & 1) == 1;
    Ok(OptionalToolDescriptorPolicy {
        kind,
        capability_mask,
        super_default,
    })
}

pub fn optional_tool_manifest_tree_supported(
    value: &str,
    vetted: &str,
    legacy: &str,
) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_optional_tool_manifest_tree_supported_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
            vetted.as_ptr() as usize as u64,
            signed_len(vetted)?,
            legacy.as_ptr() as usize as u64,
            signed_len(legacy)?,
        )
    };
    match result {
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn optional_tool_hex_encode(input: &[u8], output: &mut [u8]) -> Result<usize, MojoError> {
    let result = unsafe {
        prodex_optional_tool_hex_encode_v1(
            ABI_VERSION,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        -2 => Err(MojoError::InvalidInput),
        -3 => Err(MojoError::Capacity),
        written if written >= 0 => {
            let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
            if written == input.len().checked_mul(2).ok_or(MojoError::InvalidOutput)?
                && written <= output.len()
            {
                Ok(written)
            } else {
                Err(MojoError::InvalidOutput)
            }
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_tools_policy_preserves_aliases_descriptors_and_defaults() {
        assert_eq!(
            optional_tool_class(" CBM ").unwrap(),
            Some(OptionalToolPolicyId::CodebaseMemoryMcp)
        );
        assert_eq!(
            optional_tool_class("PLAYWRIGHT").unwrap(),
            Some(OptionalToolPolicyId::PlaywrightMcp)
        );
        assert_eq!(optional_tool_class("unknown").unwrap(), None);

        let caveman = optional_tool_descriptor_policy(OptionalToolPolicyId::Caveman).unwrap();
        assert_eq!(caveman.kind, OptionalToolPolicyKind::CodexPlugin);
        assert_eq!(caveman.capability_mask, CAP_CODEX | CAP_CLAUDE);
        assert!(caveman.super_default);

        let presidio = optional_tool_descriptor_policy(OptionalToolPolicyId::Presidio).unwrap();
        assert_eq!(presidio.kind, OptionalToolPolicyKind::Service);
        assert_eq!(presidio.capability_mask, CAP_REDACTION);
        assert!(!presidio.super_default);

        assert!(optional_tool_manifest_tree_supported("current", "current", "legacy").unwrap());
        assert!(optional_tool_manifest_tree_supported("legacy", "current", "legacy").unwrap());
        assert!(!optional_tool_manifest_tree_supported("CURRENT", "current", "legacy").unwrap());
    }
}
