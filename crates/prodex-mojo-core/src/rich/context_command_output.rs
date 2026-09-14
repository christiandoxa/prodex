use super::{
    MojoError, RICH_ABI_VERSION, RichStringView, ensure_rich_abi, mojo_mut_pointer_address,
    mojo_pointer_address, view,
};

const CONTEXT_COMMAND_OUTPUT_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Complete deterministic command-output formatter selected at the Rust host boundary.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextCommandOutputOperation {
    GitStatus = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct ContextCommandOutputFfiInput {
    operation: i64,
    max_path_entries: i64,
    input: RichStringView,
}

const _: () = assert!(std::mem::size_of::<ContextCommandOutputFfiInput>() == 32);

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct ContextCommandOutputRecord {
    hash: u64,
    category: i64,
    offset: i64,
    len: i64,
}

const _: () = assert!(std::mem::size_of::<ContextCommandOutputRecord>() == 32);

unsafe extern "C" {
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
    if input.len() > CONTEXT_COMMAND_OUTPUT_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    // ponytail: fixed 2x writer headroom; add a sizing pass if a future formatter exceeds it.
    let capacity = input
        .len()
        .checked_mul(2)
        .and_then(|value| value.checked_add(4096))
        .ok_or(MojoError::InvalidInput)?;
    let ffi_input = ContextCommandOutputFfiInput {
        operation: operation as i64,
        max_path_entries: i64::try_from(max_path_entries).unwrap_or(i64::MAX),
        input: view(input),
    };
    let line_count = if input.is_empty() {
        0
    } else {
        input.trim_end_matches('\n').split('\n').count()
    };
    let record_capacity = line_count
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or(MojoError::InvalidInput)?;
    let hash_capacity = record_capacity
        .checked_mul(2)
        .and_then(usize::checked_next_power_of_two)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut records = vec![ContextCommandOutputRecord::default(); record_capacity];
    let mut scratch = vec![0_u8; capacity];
    let mut hash_slots = vec![0_i64; hash_capacity];
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
