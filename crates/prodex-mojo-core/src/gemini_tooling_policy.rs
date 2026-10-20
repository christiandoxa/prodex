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
    fn prodex_gemini_apply_patch_sources_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_gemini_unsupported_tool_retry_choice_v1(
        abi_version: i64,
        rejected_mask: i64,
        attempted_mask: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_gemini_rtk_shell_command_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;

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

/// Number of borrow-only patch argument slots in the compiled selector.
pub const GEMINI_PATCH_SOURCE_FIELD_COUNT: usize = 17;

/// Compiled source precedence for explicit patches, edit args and content.
/// The host applies the selected strings to existing Mojo patch formatters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeminiApplyPatchSources<'a> {
    pub explicit: Option<&'a str>,
    pub path: Option<&'a str>,
    pub old: Option<&'a str>,
    pub new: Option<&'a str>,
    pub content: Option<&'a str>,
}

pub fn gemini_apply_patch_sources<'a>(
    fields: &[Option<&'a str>; GEMINI_PATCH_SOURCE_FIELD_COUNT],
) -> Result<GeminiApplyPatchSources<'a>, MojoError> {
    let mut rows = Vec::<u64>::with_capacity(GEMINI_PATCH_SOURCE_FIELD_COUNT * 3);
    for field in fields {
        if let Some(value) = field {
            rows.extend([
                1,
                value.as_ptr() as usize as u64,
                u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            ]);
        } else {
            rows.extend([0, 0, 0]);
        }
    }
    let mut result = [-7_i64; 7];
    status(unsafe {
        prodex_gemini_apply_patch_sources_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            GEMINI_PATCH_SOURCE_FIELD_COUNT as i64,
            result.as_mut_ptr() as usize as u64,
        )
    })?;
    let lookup = |index: i64, first: usize, end: usize| -> Result<Option<&'a str>, MojoError> {
        if index == -1 {
            return Ok(None);
        }
        let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
        if !(first..end).contains(&index) {
            return Err(MojoError::InvalidOutput);
        }
        fields[index].map(Some).ok_or(MojoError::InvalidOutput)
    };
    let explicit = lookup(result[0], 0, 5)?;
    let path = lookup(result[1], 5, 8)?;
    let path = if let Some(path) = path {
        let start = usize::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?;
        let end = usize::try_from(result[3]).map_err(|_| MojoError::InvalidOutput)?;
        if start >= end {
            return Err(MojoError::InvalidOutput);
        }
        Some(path.get(start..end).ok_or(MojoError::InvalidOutput)?)
    } else if result[2] == 0 && result[3] == 0 {
        None
    } else {
        return Err(MojoError::InvalidOutput);
    };
    let old = lookup(result[4], 8, 12)?;
    let new = lookup(result[5], 12, 17)?;
    let content = lookup(result[6], 16, 17)?;
    Ok(GeminiApplyPatchSources {
        explicit,
        path,
        old,
        new,
        content,
    })
}

/// Choose the next provider-rejected builtin tool with a compiled Mojo
/// priority rule. Input mask bits correspond to the Gemini bridge's fixed
/// computerUse, codeExecution, urlContext, googleSearch label projection.
pub fn gemini_unsupported_tool_retry_choice(
    rejected_mask: u8,
    attempted_mask: u8,
) -> Result<Option<usize>, MojoError> {
    let mut result = -2_i64;
    status(unsafe {
        prodex_gemini_unsupported_tool_retry_choice_v1(
            ABI_VERSION,
            i64::from(rejected_mask),
            i64::from(attempted_mask),
            (&mut result as *mut i64) as usize as u64,
        )
    })?;
    match result {
        -1 => Ok(None),
        0..=3 if rejected_mask & (1 << result) != 0 && attempted_mask & (1 << result) == 0 => Ok(
            Some(usize::try_from(result).map_err(|_| MojoError::InvalidOutput)?),
        ),
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

/// Preserve Gemini-specific raw-shell selection while delegating the complete
/// command scan and insertion to the compiled tooling policy.
pub fn gemini_rtk_shell_command(command: &str) -> Result<Option<String>, MojoError> {
    let capacity = command
        .len()
        .checked_add(4)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(capacity, 0_u8);
    let mut written = -1_i64;
    status(unsafe {
        prodex_gemini_rtk_shell_command_v1(
            ABI_VERSION,
            command.as_ptr() as u64,
            signed(command.len())?,
            output.as_mut_ptr() as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written == 0 {
        return Ok(None);
    }
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output)
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod builtin_retry_tests {
    use super::*;

    #[test]
    fn compiled_gemini_builtin_retry_chooses_priority_and_exhausts() {
        for (rejected, attempted, expected) in [
            (0, 0, None),
            (0b1111, 0, Some(0)),
            (0b1111, 0b0001, Some(1)),
            (0b1111, 0b0011, Some(2)),
            (0b1111, 0b0111, Some(3)),
            (0b1111, 0b1111, None),
            (0b1010, 0, Some(1)),
            (0b1010, 0b0010, Some(3)),
            (0b1010, 0b1010, None),
            (0b1000, 0b0111, Some(3)),
        ] {
            assert_eq!(
                gemini_unsupported_tool_retry_choice(rejected, attempted).unwrap(),
                expected,
                "{rejected:04b}/{attempted:04b}"
            );
        }
    }

    #[test]
    fn compiled_gemini_builtin_retry_abi_rejects_invalid_masks_without_mutation() {
        let mut output = -7_i64;
        let target = (&mut output as *mut i64) as usize as u64;
        unsafe {
            assert_eq!(
                prodex_gemini_unsupported_tool_retry_choice_v1(2, 1, 0, target),
                4
            );
            assert_eq!(
                prodex_gemini_unsupported_tool_retry_choice_v1(1, -1, 0, target),
                1
            );
            assert_eq!(
                prodex_gemini_unsupported_tool_retry_choice_v1(1, 16, 0, target),
                1
            );
            assert_eq!(
                prodex_gemini_unsupported_tool_retry_choice_v1(1, 1, 16, target),
                1
            );
            assert_eq!(
                prodex_gemini_unsupported_tool_retry_choice_v1(1, 1, 0, 0),
                1
            );
        }
        assert_eq!(output, -7);
        assert!(matches!(
            gemini_unsupported_tool_retry_choice(0b10000, 0),
            Err(MojoError::InvalidInput),
        ));
        assert!(matches!(
            gemini_unsupported_tool_retry_choice(0, 0b10000),
            Err(MojoError::InvalidInput),
        ));
    }
}

#[cfg(test)]
mod rtk_policy_tests {
    use super::*;

    #[test]
    fn gemini_rtk_mojo_preserves_raw_word_policy_and_wrapping() {
        for (input, expected) in [
            ("git status", Some("rtk git status")),
            ("git branch", None),
            ("echo cargo test", None),
            ("cd /repo && cargo test", Some("cd /repo && rtk cargo test")),
            (
                "X=é　/usr/bin/cargo (test)",
                Some("X=é　rtk /usr/bin/cargo (test)"),
            ),
            (
                "rtk git status || npm test",
                Some("rtk git status || rtk npm test"),
            ),
            ("X=1 rtk cargo test", None),
            ("claw-compactor compact", Some("rtk claw-compactor compact")),
            (
                "claw-compactor summarize",
                Some("rtk claw-compactor summarize"),
            ),
            (
                "prodex-claw-compactor-auto",
                Some("rtk prodex-claw-compactor-auto"),
            ),
            ("'cargo' test", None),
            ("", None),
        ] {
            assert_eq!(
                gemini_rtk_shell_command(input).unwrap().as_deref(),
                expected,
                "{input:?}"
            );
        }
        let long = format!("/{}cargo test", "x/".repeat(5000));
        assert_eq!(
            gemini_rtk_shell_command(&long).unwrap(),
            Some(format!("rtk {long}"))
        );
    }

    #[test]
    fn gemini_rtk_mojo_validates_abi_and_capacity_before_emitting_output() {
        let input = b"cargo test";
        let mut output = [77_u8; 16];
        let mut written = 99_i64;
        let mut call = |abi, capacity| unsafe {
            prodex_gemini_rtk_shell_command_v1(
                abi,
                input.as_ptr() as u64,
                input.len() as i64,
                output.as_mut_ptr() as u64,
                capacity,
                (&mut written as *mut i64) as u64,
            )
        };
        assert_eq!(call(2, 16), 4);
        assert_eq!(call(1, 13), 3);
        assert_eq!(output, [77; 16]);
        assert_eq!(written, 99);
    }
}

#[cfg(test)]
mod patch_sources_tests {
    use super::*;

    fn empty<'a>() -> [Option<&'a str>; GEMINI_PATCH_SOURCE_FIELD_COUNT] {
        [None; GEMINI_PATCH_SOURCE_FIELD_COUNT]
    }

    #[test]
    fn compiled_gemini_patch_sources_preserve_alias_priority_empty_values_and_unicode() {
        let mut fields = empty();
        assert_eq!(
            gemini_apply_patch_sources(&fields),
            Ok(GeminiApplyPatchSources {
                explicit: None,
                path: None,
                old: None,
                new: None,
                content: None,
            })
        );
        fields[1] = Some("");
        fields[2] = Some("second");
        assert_eq!(
            gemini_apply_patch_sources(&fields).unwrap().explicit,
            Some("")
        );
        fields[0] = Some("direct");
        assert_eq!(
            gemini_apply_patch_sources(&fields).unwrap().explicit,
            Some("direct")
        );
        fields[0] = None;
        fields[1] = None;
        fields[5] = Some(" \u{3000} ");
        fields[6] = Some("fallback/path.rs");
        assert_eq!(gemini_apply_patch_sources(&fields).unwrap().path, None);
        fields[5] = Some(" \u{2003}docs/雪.rs\t ");
        fields[8] = Some("old");
        fields[9] = Some("ignored old");
        fields[12] = Some("");
        fields[13] = Some("ignored new");
        fields[16] = Some("content fallback");
        let selected = gemini_apply_patch_sources(&fields).unwrap();
        assert_eq!(selected.path, Some("docs/雪.rs"));
        assert_eq!(selected.old, Some("old"));
        assert_eq!(selected.new, Some(""));
        assert_eq!(selected.content, Some("content fallback"));
        assert_eq!(selected.explicit, Some("second"));
        fields[2] = None;
        assert_eq!(gemini_apply_patch_sources(&fields).unwrap().explicit, None);
    }

    #[test]
    fn compiled_gemini_patch_sources_abi_rejects_bad_late_rows_before_output_write() {
        let mut words = [0_u64; GEMINI_PATCH_SOURCE_FIELD_COUNT * 3];
        let mut output = [79_i64; 7];
        let fields = words.as_ptr() as usize as u64;
        let result = output.as_mut_ptr() as usize as u64;
        for (version, data, count, destination, expected) in [
            (2, fields, 17, result, 4),
            (1, 0, 17, result, 1),
            (1, fields, 16, result, 1),
            (1, fields, 18, result, 1),
            (1, fields, 17, 0, 1),
        ] {
            assert_eq!(
                unsafe { prodex_gemini_apply_patch_sources_v1(version, data, count, destination) },
                expected
            );
            assert_eq!(output, [79; 7]);
        }
        words[16 * 3] = 2;
        assert_eq!(
            unsafe {
                prodex_gemini_apply_patch_sources_v1(1, words.as_ptr() as usize as u64, 17, result)
            },
            1
        );
        assert_eq!(output, [79; 7]);
        words[16 * 3] = 1;
        words[16 * 3 + 1] = 0;
        words[16 * 3 + 2] = 1;
        assert_eq!(
            unsafe {
                prodex_gemini_apply_patch_sources_v1(1, words.as_ptr() as usize as u64, 17, result)
            },
            1
        );
        assert_eq!(output, [79; 7]);
    }
}
