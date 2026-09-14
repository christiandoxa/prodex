use super::{
    MojoError, RICH_ABI_VERSION, RichStringView, ensure_rich_abi, hash_capacity,
    mojo_mut_pointer_address, mojo_pointer_address, view,
};

/// Complete deterministic command-output formatter selected at the Rust host boundary.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextCommandOutputOperation {
    GitStatus = 1,
    FileList = 2,
    Search = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ContextCommandOutputFfiInput {
    operation: i64,
    max_path_entries: u64,
    max_lines: u64,
    max_line_chars: u64,
    max_search_matches: u64,
    input: RichStringView,
}

const _: () = assert!(std::mem::size_of::<ContextCommandOutputFfiInput>() == 56);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct ContextCommandOutputRecord {
    occurrences: u64,
    category: i64,
    offset: i64,
    len: i64,
}

const _: () = assert!(std::mem::size_of::<ContextCommandOutputRecord>() == 32);

unsafe extern "C" {
    fn prodex_mojo_context_command_output_size_v1(
        abi_version: i64,
        input: u64,
        meaningful_lines: u64,
        meaningful_bytes: u64,
    ) -> i64;

    fn prodex_mojo_context_command_output_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        records: u64,
        record_capacity: i64,
        scratch: u64,
        scratch_capacity: i64,
        hash_slots: u64,
        hash_capacity: i64,
        written: u64,
    ) -> i64;

    fn prodex_mojo_context_search_output_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        records: u64,
        record_capacity: i64,
        scratch: u64,
        scratch_capacity: i64,
        hash_slots: u64,
        hash_capacity: i64,
        path_output: u64,
        path_capacity: i64,
        text_output: u64,
        text_capacity: i64,
        written: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy)]
struct ContextCommandOutputAllocation {
    output: usize,
    records: usize,
    scratch: usize,
    hash_slots: usize,
}

fn allocation_from_counts(
    operation: i64,
    meaningful_lines: usize,
    meaningful_bytes: usize,
) -> Result<ContextCommandOutputAllocation, MojoError> {
    let multiplier = if matches!(operation, 2 | 3) { 3 } else { 2 };
    let records = meaningful_lines
        .checked_mul(multiplier)
        .and_then(|value| value.checked_add(1))
        .ok_or(MojoError::InvalidInput)?;
    let scratch = meaningful_bytes
        .checked_mul(multiplier)
        .and_then(|value| value.checked_add(1))
        .ok_or(MojoError::InvalidInput)?;
    let output = scratch
        .checked_add(records.checked_mul(2).ok_or(MojoError::InvalidInput)?)
        .and_then(|value| value.checked_add(1024))
        .ok_or(MojoError::InvalidInput)?;
    Ok(ContextCommandOutputAllocation {
        output,
        records,
        scratch,
        hash_slots: hash_capacity(records)?,
    })
}

fn allocation_plan(
    input: &ContextCommandOutputFfiInput,
) -> Result<ContextCommandOutputAllocation, MojoError> {
    let mut meaningful_lines = 0_i64;
    let mut meaningful_bytes = 0_i64;
    let status = unsafe {
        prodex_mojo_context_command_output_size_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(input),
            mojo_mut_pointer_address(&mut meaningful_lines),
            mojo_mut_pointer_address(&mut meaningful_bytes),
        )
    };
    if status != 0 {
        return Err(super::status_error(status, 10, 1, 0, 0));
    }
    allocation_from_counts(
        input.operation,
        usize::try_from(meaningful_lines).map_err(|_| MojoError::InvalidOutput)?,
        usize::try_from(meaningful_bytes).map_err(|_| MojoError::InvalidOutput)?,
    )
}

fn zeroed<T: Clone + Default>(len: usize) -> Result<Vec<T>, MojoError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| MojoError::InvalidInput)?;
    values.resize(len, T::default());
    Ok(values)
}

/// Formats one normalized command output in Mojo.
///
/// A successful `None` means the selected formatter found no matching structure and the
/// caller should apply its existing generic truncation policy.
pub fn context_command_output(
    operation: ContextCommandOutputOperation,
    input: &str,
    max_path_entries: usize,
) -> Result<Option<String>, MojoError> {
    ensure_rich_abi()?;
    if input.len() > i64::MAX as usize {
        return Err(MojoError::InvalidInput);
    }
    let ffi_input = ContextCommandOutputFfiInput {
        operation: operation as i64,
        max_path_entries: max_path_entries as u64,
        max_lines: 0,
        max_line_chars: 0,
        max_search_matches: 0,
        input: view(input),
    };
    context_command_output_ffi(ffi_input)
}

fn context_command_output_ffi(
    ffi_input: ContextCommandOutputFfiInput,
) -> Result<Option<String>, MojoError> {
    let allocation = allocation_plan(&ffi_input)?;
    let mut output = zeroed::<u8>(allocation.output)?;
    let mut records = zeroed::<ContextCommandOutputRecord>(allocation.records)?;
    let mut scratch = zeroed::<u8>(allocation.scratch)?;
    let mut hash_slots = zeroed::<i64>(allocation.hash_slots)?;
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_context_command_output_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(records.as_mut_ptr()),
            i64::try_from(records.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(scratch.as_mut_ptr()),
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(hash_slots.as_mut_ptr()),
            i64::try_from(hash_slots.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status == 5 {
        return Ok(None);
    }
    if status != 0 {
        return Err(super::status_error(status, 10, 1, 0, 0));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output)
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

pub fn context_file_list_output(
    input: &str,
    max_lines: usize,
    max_line_chars: usize,
    max_path_entries: usize,
) -> Result<Option<String>, MojoError> {
    ensure_rich_abi()?;
    if input.len() > i64::MAX as usize {
        return Err(MojoError::InvalidInput);
    }
    let ffi_input = ContextCommandOutputFfiInput {
        operation: ContextCommandOutputOperation::FileList as i64,
        max_path_entries: max_path_entries as u64,
        max_lines: max_lines as u64,
        max_line_chars: max_line_chars as u64,
        max_search_matches: 0,
        input: view(input),
    };
    context_command_output_ffi(ffi_input)
}

pub fn context_search_output(
    input: &str,
    max_lines: usize,
    max_line_chars: usize,
    max_search_matches: usize,
) -> Result<Option<String>, MojoError> {
    ensure_rich_abi()?;
    if input.len() > i64::MAX as usize {
        return Err(MojoError::InvalidInput);
    }
    let ffi_input = ContextCommandOutputFfiInput {
        operation: ContextCommandOutputOperation::Search as i64,
        max_path_entries: 0,
        max_lines: max_lines as u64,
        max_line_chars: max_line_chars as u64,
        max_search_matches: max_search_matches as u64,
        input: view(input),
    };
    let allocation = allocation_plan(&ffi_input)?;
    let temporary_capacity = input
        .split('\n')
        .map(str::len)
        .max()
        .unwrap_or_default()
        .max(1);
    let mut output = zeroed::<u8>(allocation.output)?;
    let mut records = zeroed::<ContextCommandOutputRecord>(allocation.records)?;
    let mut scratch = zeroed::<u8>(allocation.scratch)?;
    let mut hash_slots = zeroed::<i64>(allocation.hash_slots)?;
    let mut path_output = zeroed::<u8>(temporary_capacity)?;
    let mut text_output = zeroed::<u8>(temporary_capacity)?;
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_context_search_output_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(records.as_mut_ptr()),
            i64::try_from(records.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(scratch.as_mut_ptr()),
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(hash_slots.as_mut_ptr()),
            i64::try_from(hash_slots.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(path_output.as_mut_ptr()),
            i64::try_from(path_output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(text_output.as_mut_ptr()),
            i64::try_from(text_output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status == 5 {
        return Ok(None);
    }
    if status != 0 {
        return Err(super::status_error(status, 10, 3, 0, 0));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output)
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizing_keeps_blank_heavy_inputs_bounded() {
        let input = "\n".repeat(64 * 1024 * 1024 - 1) + "x";
        let ffi_input = ContextCommandOutputFfiInput {
            operation: ContextCommandOutputOperation::GitStatus as i64,
            max_path_entries: 120,
            max_lines: 0,
            max_line_chars: 0,
            max_search_matches: 0,
            input: view(&input),
        };
        let allocation = allocation_plan(&ffi_input).unwrap();
        assert_eq!(allocation.records, 3);
        assert_eq!(allocation.hash_slots, 8);
        assert!(allocation.output + allocation.scratch < 2048);
        assert_eq!(
            context_command_output(ContextCommandOutputOperation::GitStatus, &input, 120).unwrap(),
            Some("sum: git status\nother (1): x\n".to_string())
        );
    }

    #[test]
    fn file_list_sizing_keeps_blank_heavy_inputs_bounded() {
        let input = "\n".repeat(4 * 1024 * 1024 - 1) + "x";
        let ffi_input = ContextCommandOutputFfiInput {
            operation: ContextCommandOutputOperation::FileList as i64,
            max_path_entries: 120,
            max_lines: 1_000,
            max_line_chars: 240,
            max_search_matches: 0,
            input: view(&input),
        };
        let allocation = allocation_plan(&ffi_input).unwrap();
        assert_eq!(allocation.records, 4);
        assert_eq!(allocation.hash_slots, 8);
        assert!(allocation.output + allocation.scratch < 2048);
        assert_eq!(
            context_file_list_output(&input, 1_000, 240, 120).unwrap(),
            None
        );
    }

    #[test]
    fn sizing_arithmetic_rejects_unrepresentable_capacity() {
        assert_eq!(
            allocation_from_counts(
                ContextCommandOutputOperation::FileList as i64,
                usize::MAX,
                usize::MAX
            )
            .unwrap_err(),
            MojoError::InvalidInput
        );
    }

    #[test]
    fn raw_boundary_rejects_invalid_inputs_and_capacity() {
        let invalid = [0xff_u8];
        let mut input = ContextCommandOutputFfiInput {
            operation: ContextCommandOutputOperation::GitStatus as i64,
            max_path_entries: 120,
            max_lines: 0,
            max_line_chars: 0,
            max_search_matches: 0,
            input: RichStringView {
                ptr: mojo_pointer_address(invalid.as_ptr()),
                len: 1,
            },
        };
        let mut lines = 0_i64;
        let mut bytes = 0_i64;
        let mut size = |abi, input_address| unsafe {
            prodex_mojo_context_command_output_size_v1(
                abi,
                input_address,
                mojo_mut_pointer_address(&mut lines),
                mojo_mut_pointer_address(&mut bytes),
            )
        };
        assert_eq!(size(RICH_ABI_VERSION + 1, mojo_pointer_address(&input)), 4);
        assert_eq!(size(RICH_ABI_VERSION, 0), 1);
        assert_eq!(size(RICH_ABI_VERSION, mojo_pointer_address(&input)), 2);

        input.input = view("## main\n?? one\n");
        let allocation = allocation_plan(&input).unwrap();
        let mut output = [0_u8; 1];
        let mut records = zeroed::<ContextCommandOutputRecord>(allocation.records).unwrap();
        let mut scratch = zeroed::<u8>(allocation.scratch).unwrap();
        let mut hash_slots = zeroed::<i64>(allocation.hash_slots).unwrap();
        let mut written = 0_i64;
        let status = unsafe {
            prodex_mojo_context_command_output_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&input),
                mojo_mut_pointer_address(output.as_mut_ptr()),
                1,
                mojo_mut_pointer_address(records.as_mut_ptr()),
                records.len() as i64,
                mojo_mut_pointer_address(scratch.as_mut_ptr()),
                scratch.len() as i64,
                mojo_mut_pointer_address(hash_slots.as_mut_ptr()),
                hash_slots.len() as i64,
                mojo_mut_pointer_address(&mut written),
            )
        };
        assert_eq!(status, 3);
    }

    #[test]
    fn raw_search_boundary_rejects_invalid_inputs_and_capacity() {
        let invalid = [0xff_u8];
        let mut input = ContextCommandOutputFfiInput {
            operation: ContextCommandOutputOperation::Search as i64,
            max_path_entries: 0,
            max_lines: 1_000,
            max_line_chars: 240,
            max_search_matches: 4,
            input: RichStringView {
                ptr: mojo_pointer_address(invalid.as_ptr()),
                len: 1,
            },
        };
        let mut output = [0_u8; 1];
        let mut records = [ContextCommandOutputRecord::default(); 4];
        let mut scratch = [0_u8; 64];
        let mut hash_slots = [0_i64; 16];
        let mut path = [0_u8; 32];
        let mut text = [0_u8; 32];
        let mut written = 0_i64;
        let mut call = |abi, input_address| unsafe {
            prodex_mojo_context_search_output_v1(
                abi,
                input_address,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                1,
                mojo_mut_pointer_address(records.as_mut_ptr()),
                records.len() as i64,
                mojo_mut_pointer_address(scratch.as_mut_ptr()),
                scratch.len() as i64,
                mojo_mut_pointer_address(hash_slots.as_mut_ptr()),
                hash_slots.len() as i64,
                mojo_mut_pointer_address(path.as_mut_ptr()),
                path.len() as i64,
                mojo_mut_pointer_address(text.as_mut_ptr()),
                text.len() as i64,
                mojo_mut_pointer_address(&mut written),
            )
        };
        assert_eq!(call(RICH_ABI_VERSION + 1, mojo_pointer_address(&input)), 4);
        assert_eq!(call(RICH_ABI_VERSION, 0), 1);
        assert_eq!(call(RICH_ABI_VERSION, mojo_pointer_address(&input)), 2);

        input.input = view("src/lib.rs:1:test\n");
        assert_eq!(call(RICH_ABI_VERSION, mojo_pointer_address(&input)), 3);
    }
}
