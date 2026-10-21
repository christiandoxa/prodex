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
#[path = "../tests/src/gemini_tooling_policy/tests.rs"]
mod tests;

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
#[path = "../tests/src/gemini_tooling_policy/builtin_retry_tests.rs"]
mod builtin_retry_tests;

#[cfg(test)]
#[path = "../tests/src/gemini_tooling_policy/rtk_policy_tests.rs"]
mod rtk_policy_tests;

#[cfg(test)]
#[path = "../tests/src/gemini_tooling_policy/patch_sources_tests.rs"]
mod patch_sources_tests;
