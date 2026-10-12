use crate::MojoError;

const ABI_VERSION: i64 = 1;
const FLAG_MUTATING: i64 = 1;
const FLAG_CANONICAL_EXEC: i64 = 2;
const GEMINI_PATCH_BLOCK_STATUS_NO_MATCH: i64 = 2;
const GEMINI_PATCH_BLOCK_STATUS_CAPACITY: i64 = 3;
const GEMINI_EDIT_PATCH_STATUS_NO_MATCH: i64 = 2;
const GEMINI_EDIT_PATCH_STATUS_CAPACITY: i64 = 3;

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
    fn prodex_gemini_unified_diff_to_apply_patch_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_gemini_apply_patch_block_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_gemini_edit_patch_v1(
        abi_version: i64,
        path_address: u64,
        path_length: i64,
        old_address: u64,
        old_length: i64,
        old_present: i64,
        new_address: u64,
        new_length: i64,
        new_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
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

pub fn gemini_unified_diff_to_apply_patch(input: &str) -> Result<Option<String>, MojoError> {
    const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
    let mut capacity = input.len().saturating_mul(2).saturating_add(4096).max(4096);
    capacity = capacity.min(MAX_OUTPUT_BYTES);
    loop {
        let mut output = vec![0_u8; capacity];
        let mut written = 0_i64;
        let result = unsafe {
            prodex_gemini_unified_diff_to_apply_patch_v1(
                ABI_VERSION,
                input.as_ptr() as usize as u64,
                signed(input.len())?,
                output.as_mut_ptr() as usize as u64,
                signed(output.len())?,
                (&mut written as *mut i64) as usize as u64,
            )
        };
        match result {
            0 => {
                let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
                if written > output.len() {
                    return Err(MojoError::InvalidOutput);
                }
                output.truncate(written);
                return String::from_utf8(output)
                    .map(Some)
                    .map_err(|_| MojoError::InvalidOutput);
            }
            2 => return Ok(None),
            3 if capacity < MAX_OUTPUT_BYTES => {
                capacity = capacity.saturating_mul(2).min(MAX_OUTPUT_BYTES);
            }
            1 => return Err(MojoError::InvalidInput),
            3 => return Err(MojoError::Capacity),
            4 => return Err(MojoError::AbiMismatch),
            _ => return Err(MojoError::InvalidOutput),
        }
    }
}

/// Extracts and repairs one `*** Begin Patch` block through the Mojo policy.
pub fn gemini_apply_patch_block(input: &str) -> Result<Option<String>, MojoError> {
    const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
    if input.len() > 4 * 1024 * 1024 {
        return Err(MojoError::InvalidInput);
    }
    let mut capacity = input.len().saturating_mul(2).saturating_add(1024).max(1024);
    capacity = capacity.min(MAX_OUTPUT_BYTES);
    loop {
        let mut output = vec![0_u8; capacity];
        let mut written = 0_i64;
        let result = unsafe {
            prodex_gemini_apply_patch_block_v1(
                ABI_VERSION,
                input.as_ptr() as usize as u64,
                signed(input.len())?,
                output.as_mut_ptr() as usize as u64,
                signed(output.len())?,
                (&mut written as *mut i64) as usize as u64,
            )
        };
        match result {
            0 => {
                let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
                if written > output.len() {
                    return Err(MojoError::InvalidOutput);
                }
                output.truncate(written);
                return String::from_utf8(output)
                    .map(Some)
                    .map_err(|_| MojoError::InvalidOutput);
            }
            GEMINI_PATCH_BLOCK_STATUS_NO_MATCH => return Ok(None),
            GEMINI_PATCH_BLOCK_STATUS_CAPACITY if capacity < MAX_OUTPUT_BYTES => {
                capacity = capacity.saturating_mul(2).min(MAX_OUTPUT_BYTES);
            }
            1 => return Err(MojoError::InvalidInput),
            GEMINI_PATCH_BLOCK_STATUS_CAPACITY => return Err(MojoError::Capacity),
            4 => return Err(MojoError::AbiMismatch),
            _ => return Err(MojoError::InvalidOutput),
        }
    }
}

/// Assembles a Gemini edit-tool Add/Update patch through the Mojo policy.
pub fn gemini_edit_patch(
    path: &str,
    old: Option<&str>,
    new: Option<&str>,
) -> Result<Option<String>, MojoError> {
    const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
    const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
    let path = path.trim();
    if path.is_empty()
        || path.len() > MAX_INPUT_BYTES
        || old.is_some_and(|value| value.len() > MAX_INPUT_BYTES)
        || new.is_some_and(|value| value.len() > MAX_INPUT_BYTES)
    {
        return Ok(None);
    }
    let mut capacity = path
        .len()
        .saturating_add(old.map_or(0, str::len))
        .saturating_add(new.map_or(0, str::len))
        .saturating_add(128)
        .clamp(128, MAX_OUTPUT_BYTES);
    loop {
        let mut output = vec![0_u8; capacity];
        let mut written = 0_i64;
        let result = unsafe {
            prodex_gemini_edit_patch_v1(
                ABI_VERSION,
                path.as_ptr() as usize as u64,
                signed(path.len())?,
                old.map_or(0, |value| value.as_ptr() as usize as u64),
                signed(old.map_or(0, str::len))?,
                i64::from(old.is_some()),
                new.map_or(0, |value| value.as_ptr() as usize as u64),
                signed(new.map_or(0, str::len))?,
                i64::from(new.is_some()),
                output.as_mut_ptr() as usize as u64,
                signed(output.len())?,
                (&mut written as *mut i64) as usize as u64,
            )
        };
        match result {
            0 => {
                let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
                if written > output.len() {
                    return Err(MojoError::InvalidOutput);
                }
                output.truncate(written);
                return String::from_utf8(output)
                    .map(Some)
                    .map_err(|_| MojoError::InvalidOutput);
            }
            GEMINI_EDIT_PATCH_STATUS_NO_MATCH => return Ok(None),
            GEMINI_EDIT_PATCH_STATUS_CAPACITY if capacity < MAX_OUTPUT_BYTES => {
                capacity = capacity.saturating_mul(2).min(MAX_OUTPUT_BYTES);
            }
            1 => return Err(MojoError::InvalidInput),
            GEMINI_EDIT_PATCH_STATUS_CAPACITY => return Err(MojoError::Capacity),
            4 => return Err(MojoError::AbiMismatch),
            _ => return Err(MojoError::InvalidOutput),
        }
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
    fn gemini_unified_diff_kernel_converts_update_add_delete_and_move() {
        let input = concat!(
            "diff --git a/a.txt b/b.txt\n",
            "--- a/a.txt\n",
            "+++ b/b.txt\n",
            "@@ -1 +1 @@ context\n",
            "-old\n",
            "+new\n",
            "diff --git a/new.txt b/new.txt\n",
            "--- /dev/null\n",
            "+++ b/new.txt\n",
            "@@ -0,0 +1 @@\n",
            "+hello\n",
            "diff --git a/gone.txt b/gone.txt\n",
            "--- a/gone.txt\n",
            "+++ /dev/null\n",
        );
        assert_eq!(
            gemini_unified_diff_to_apply_patch(input).unwrap().unwrap(),
            concat!(
                "*** Begin Patch\n",
                "*** Update File: a.txt\n",
                "*** Move to: b.txt\n",
                "@@ context\n",
                "-old\n",
                "+new\n",
                "*** Add File: new.txt\n",
                "+hello\n",
                "*** Delete File: gone.txt\n",
                "*** End Patch",
            )
        );
        assert_eq!(
            gemini_unified_diff_to_apply_patch("plain text").unwrap(),
            None
        );
    }

    #[test]
    fn gemini_apply_patch_block_kernel_normalizes_newlines_and_add_file_lines() {
        let input = "prefix\r\n  *** Begin Patch\r\n*** Add File: note.txt\r\nhello\r\n+world\r\n*** End Patch\r\ntrailer";
        assert_eq!(
            gemini_apply_patch_block(input).unwrap(),
            Some(
                "  *** Begin Patch\n*** Add File: note.txt\n+hello\n+world\n*** End Patch"
                    .to_string()
            )
        );
        assert_eq!(gemini_apply_patch_block("plain text").unwrap(), None);
    }

    #[test]
    fn gemini_edit_patch_kernel_selects_add_update_and_no_match() {
        assert_eq!(
            gemini_edit_patch("note.txt", None, Some("hello\r\nworld")).unwrap(),
            Some(
                "*** Begin Patch\n*** Add File: note.txt\n+hello\n+world\n*** End Patch"
                    .to_string()
            )
        );
        assert_eq!(
            gemini_edit_patch("note.txt", Some("old"), Some("new")).unwrap(),
            Some(
                "*** Begin Patch\n*** Update File: note.txt\n@@\n-old\n+new\n*** End Patch"
                    .to_string()
            )
        );
        assert_eq!(
            gemini_edit_patch("note.txt", Some("old"), None).unwrap(),
            None
        );
        assert_eq!(gemini_edit_patch("  ", None, Some("new")).unwrap(), None);
    }

    #[test]
    fn gemini_tooling_model_policy_preserves_gemini3_detection() {
        assert!(gemini_model_uses_gemini3_toolset("GEMINI-3-PRO").unwrap());
        assert!(gemini_model_uses_gemini3_toolset("auto").unwrap());
        assert!(gemini_model_uses_gemini3_toolset("AUTO-GEMINI-3").unwrap());
        assert!(!gemini_model_uses_gemini3_toolset("gemini-2.5-pro").unwrap());
    }
}
