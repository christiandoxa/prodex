use crate::MojoError;

const ABI_VERSION: i64 = 1;
const FLAG_MUTATING: i64 = 1;
const FLAG_CANONICAL_EXEC: i64 = 2;

const ALIASES: &[(i64, &str)] = &[
    (1 << 0, "exec_command"),
    (1 << 1, "run_shell_command"),
    (1 << 2, "shell"),
    (1 << 3, "bash"),
    (1 << 4, "apply_patch"),
    (1 << 5, "edit"),
    (1 << 6, "replace"),
    (1 << 7, "read_file"),
    (1 << 8, "read"),
    (1 << 9, "read_many_files"),
    (1 << 10, "glob"),
    (1 << 11, "grep"),
    (1 << 12, "rip_grep"),
    (1 << 13, "rg"),
    (1 << 14, "search"),
    (1 << 15, "write_file"),
    (1 << 16, "write"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiToolNamePolicy {
    pub normalized: String,
    pub suffix: String,
    pub aliases: Vec<&'static str>,
    pub mutating: bool,
    pub canonical_exec: bool,
}

unsafe extern "C" {
    fn prodex_gemini_tool_name_policy_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        suffix_start_address: u64,
        alias_mask_address: u64,
        flags_address: u64,
    ) -> i64;

    fn prodex_gemini_tooling_model_policy_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
    ) -> i64;
}

fn signed(value: usize) -> Result<i64, MojoError> {
    i64::try_from(value).map_err(|_| MojoError::InvalidInput)
}

fn status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn gemini_tool_name_policy(name: &str) -> Result<GeminiToolNamePolicy, MojoError> {
    let mut output = vec![0_u8; name.len().max(1)];
    let mut written = 0_i64;
    let mut suffix_start = 0_i64;
    let mut alias_mask = 0_i64;
    let mut flags = 0_i64;
    status(unsafe {
        prodex_gemini_tool_name_policy_v1(
            ABI_VERSION,
            name.as_ptr() as usize as u64,
            signed(name.len())?,
            output.as_mut_ptr() as usize as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as usize as u64,
            (&mut suffix_start as *mut i64) as usize as u64,
            (&mut alias_mask as *mut i64) as usize as u64,
            (&mut flags as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let suffix_start = usize::try_from(suffix_start).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() || suffix_start > written || alias_mask < 0 {
        return Err(MojoError::InvalidOutput);
    }
    let normalized =
        String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)?;
    let suffix = String::from_utf8(output[suffix_start..written].to_vec())
        .map_err(|_| MojoError::InvalidOutput)?;
    let aliases = ALIASES
        .iter()
        .filter_map(|(bit, alias)| ((alias_mask & bit) != 0).then_some(*alias))
        .collect();
    Ok(GeminiToolNamePolicy {
        normalized,
        suffix,
        aliases,
        mutating: (flags & FLAG_MUTATING) != 0,
        canonical_exec: (flags & FLAG_CANONICAL_EXEC) != 0,
    })
}

pub fn gemini_model_uses_gemini3_toolset(model: &str) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_gemini_tooling_model_policy_v1(
            ABI_VERSION,
            model.as_ptr() as usize as u64,
            signed(model.len())?,
        )
    };
    match result {
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_tooling_policy_preserves_normalization_aliases_and_mutation() {
        let shell = gemini_tool_name_policy("  RUN-SHELL-COMMAND  ").unwrap();
        assert_eq!(shell.normalized, "run_shell_command");
        assert_eq!(shell.suffix, "run_shell_command");
        assert!(shell.aliases.contains(&"exec_command"));
        assert!(shell.aliases.contains(&"bash"));
        assert!(shell.mutating);
        assert!(shell.canonical_exec);

        let namespaced = gemini_tool_name_policy("mcp__repo__read-file").unwrap();
        assert_eq!(namespaced.normalized, "mcp__repo__read_file");
        assert_eq!(namespaced.suffix, "read_file");
        assert!(namespaced.aliases.is_empty());
        assert!(!namespaced.mutating);
        assert!(!namespaced.canonical_exec);

        let namespaced_exec = gemini_tool_name_policy("mcp__repo__exec-command").unwrap();
        assert_eq!(namespaced_exec.suffix, "exec_command");
        assert!(namespaced_exec.mutating);
        assert!(!namespaced_exec.canonical_exec);

        let dotted = gemini_tool_name_policy("namespace.Exec-Command").unwrap();
        assert_eq!(dotted.normalized, "exec_command");
        assert!(dotted.aliases.contains(&"run_shell_command"));
        assert!(dotted.mutating);

        let empty = gemini_tool_name_policy("   ").unwrap();
        assert_eq!(empty.normalized, "");
        assert_eq!(empty.suffix, "");
        assert!(empty.aliases.is_empty());
        assert!(!empty.mutating);
    }

    #[test]
    fn gemini_tooling_model_policy_preserves_gemini3_detection() {
        assert!(gemini_model_uses_gemini3_toolset("GEMINI-3-PRO").unwrap());
        assert!(gemini_model_uses_gemini3_toolset("auto").unwrap());
        assert!(gemini_model_uses_gemini3_toolset("AUTO-GEMINI-3").unwrap());
        assert!(!gemini_model_uses_gemini3_toolset("gemini-2.5-pro").unwrap());
    }
}
