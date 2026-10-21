use super::super::{ensure_rich_abi, mojo_mut_pointer_address};
use super::SUPER_EXPOSE_ABI_VERSION;
use crate::MojoError;

const OP_PROGRAM: i64 = 0;
const OP_CWD: i64 = 1;
const OP_ARGUMENT_COUNT: i64 = 2;
const OP_ARGUMENT: i64 = 3;
const OP_ENV_COUNT: i64 = 4;
const OP_ENV_ENTRY: i64 = 5;
const OP_STDIN: i64 = 6;
const OP_TIMEOUT: i64 = 7;
const OP_TEXT_CUT: i64 = 8;
const OP_BOUNDED_TEXT: i64 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum SuperExposeExecTextKind {
    /// Validate the requested executable label.
    Program = OP_PROGRAM,
    /// Validate the requested child working directory.
    Cwd = OP_CWD,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum SuperExposeExecCountKind {
    /// Validate the number of command arguments.
    Arguments = OP_ARGUMENT_COUNT,
    /// Validate the number of environment entries.
    Environment = OP_ENV_COUNT,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeExecValidation {
    Valid,
    Invalid,
    ArgumentLimit,
    TotalArgumentLimit,
    EnvironmentKey,
    EnvironmentValue,
    CountLimit,
    StdinLimit,
    TimeoutLimit,
}

unsafe extern "C" {
    fn prodex_mojo_super_expose_optional_alias_plan_v1(
        abi_version: i64,
        alias_address: u64,
        alias_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_exec_validation_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        input_length: i64,
        secondary_address: u64,
        secondary_length: i64,
        numeric_value: u64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn call(
    operation: i64,
    input: Option<&str>,
    secondary: Option<&str>,
    numeric_value: u64,
) -> Result<[i64; 2], MojoError> {
    ensure_rich_abi()?;
    let input = input
        .map(|value| {
            Ok::<_, MojoError>((
                value.as_ptr() as usize as u64,
                i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            ))
        })
        .transpose()?
        .unwrap_or((0, 0));
    let secondary = secondary
        .map(|value| {
            Ok::<_, MojoError>((
                value.as_ptr() as usize as u64,
                i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            ))
        })
        .transpose()?
        .unwrap_or((0, 0));
    let mut output = [-1_i64; 2];
    status(unsafe {
        prodex_mojo_super_expose_exec_validation_v1(
            SUPER_EXPOSE_ABI_VERSION,
            operation,
            input.0,
            input.1,
            secondary.0,
            secondary.1,
            numeric_value,
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    })?;
    Ok(output)
}

fn decode(value: i64) -> Result<SuperExposeExecValidation, MojoError> {
    match value {
        1 => Ok(SuperExposeExecValidation::Valid),
        2 => Ok(SuperExposeExecValidation::Invalid),
        3 => Ok(SuperExposeExecValidation::TotalArgumentLimit),
        4 => Ok(SuperExposeExecValidation::ArgumentLimit),
        5 => Ok(SuperExposeExecValidation::EnvironmentKey),
        6 => Ok(SuperExposeExecValidation::EnvironmentValue),
        7 => Ok(SuperExposeExecValidation::CountLimit),
        8 => Ok(SuperExposeExecValidation::StdinLimit),
        9 => Ok(SuperExposeExecValidation::TimeoutLimit),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Plans one optional alias using canonical tool IDs and fixed prefix arguments.
/// OS paths and command arguments remain with the caller.
pub fn super_expose_optional_alias_plan(alias: &str) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    // Alias plans contain only fixed strings; caller-owned capacity is bounded.
    const PLAN_CAPACITY: usize = 1024;
    let mut output = Vec::new();
    output
        .try_reserve_exact(PLAN_CAPACITY)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(PLAN_CAPACITY, 0_u8);
    let mut written = -1_i64;
    status(unsafe {
        prodex_mojo_super_expose_optional_alias_plan_v1(
            SUPER_EXPOSE_ABI_VERSION,
            alias.as_ptr() as u64,
            i64::try_from(alias.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            PLAN_CAPACITY as i64,
            (&mut written as *mut i64) as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written == 0 || written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    std::str::from_utf8(&output).map_err(|_| MojoError::InvalidOutput)?;
    Ok(output)
}

/// Validate the bounded program or cwd text policy in compiled Mojo.
pub fn super_expose_exec_validate_text(
    value: &str,
    kind: SuperExposeExecTextKind,
) -> Result<SuperExposeExecValidation, MojoError> {
    decode(call(kind as i64, Some(value), None, 0)?[0])
}

/// Validate the bounded argument-count policy in compiled Mojo.
pub fn super_expose_exec_validate_count(
    count: usize,
    kind: SuperExposeExecCountKind,
) -> Result<SuperExposeExecValidation, MojoError> {
    decode(
        call(
            kind as i64,
            None,
            None,
            u64::try_from(count).map_err(|_| MojoError::InvalidInput)?,
        )?[0],
    )
}

/// Validate one argument and return Mojo's checked cumulative byte total.
pub fn super_expose_exec_validate_argument(
    value: &str,
    total_bytes: usize,
) -> Result<(SuperExposeExecValidation, usize), MojoError> {
    let output = call(
        OP_ARGUMENT,
        Some(value),
        None,
        u64::try_from(total_bytes).map_err(|_| MojoError::InvalidInput)?,
    )?;
    let validation = decode(output[0])?;
    let total = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    Ok((validation, total))
}

/// Validate one environment key/value pair in compiled Mojo.
pub fn super_expose_exec_validate_env_entry(
    key: &str,
    value: &str,
) -> Result<SuperExposeExecValidation, MojoError> {
    decode(call(OP_ENV_ENTRY, Some(key), Some(value), 0)?[0])
}

/// Validate the bounded stdin byte length in compiled Mojo.
pub fn super_expose_exec_validate_stdin(
    byte_length: usize,
) -> Result<SuperExposeExecValidation, MojoError> {
    decode(
        call(
            OP_STDIN,
            None,
            None,
            u64::try_from(byte_length).map_err(|_| MojoError::InvalidInput)?,
        )?[0],
    )
}

/// Validate the bounded timeout range in compiled Mojo.
pub fn super_expose_exec_validate_timeout(
    timeout_ms: u64,
) -> Result<SuperExposeExecValidation, MojoError> {
    decode(call(OP_TIMEOUT, None, None, timeout_ms)?[0])
}

/// Clips borrowed UTF-8 text to a byte budget through the shared compiled text kernel.
pub fn super_expose_text_cut(text: &str, max_bytes: usize) -> Result<usize, MojoError> {
    let [end, reserved] = call(
        OP_TEXT_CUT,
        Some(text),
        None,
        u64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
    )?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    if reserved != 0 || end > text.len() || end > max_bytes || !text.is_char_boundary(end) {
        return Err(MojoError::InvalidOutput);
    }
    if text.len() <= max_bytes && end != text.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(end)
}

/// Validates nonempty bounded text with the compiled Unicode control policy.
pub fn super_expose_bounded_text_valid(value: &str, max_bytes: usize) -> Result<bool, MojoError> {
    match call(
        OP_BOUNDED_TEXT,
        Some(value),
        None,
        u64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
    )? {
        [1, 0] => Ok(true),
        [2, 0] => Ok(false),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod alias_tests {
    use super::*;

    #[test]
    fn compiled_optional_alias_plan_preserves_exact_integrated_playwright_spelling() {
        assert_eq!(
            super_expose_optional_alias_plan("unknown").unwrap(),
            b"null"
        );
        assert_eq!(super_expose_optional_alias_plan("").unwrap(), b"null");
        assert_eq!(super_expose_optional_alias_plan(" RTK ").unwrap(), br#"{"tool":"rtk","program":{"kind":"tool_path","missing_path_message":"validated optional tool rtk has no executable path"},"prefix":[]}"#);
        let package = super_expose_optional_alias_plan("playwright-mcp").unwrap();
        assert_eq!(
            super_expose_optional_alias_plan("PLAYWRIGHT").unwrap(),
            package
        );
        assert_eq!(
            super_expose_optional_alias_plan(" playwright ").unwrap(),
            package
        );
        assert_ne!(
            super_expose_optional_alias_plan("playwright").unwrap(),
            package
        );
    }

    #[test]
    fn compiled_optional_alias_abi_rejects_capacity_utf8_and_null_input_without_mutation() {
        let mut output = [83_u8; 512];
        let mut written = 71_i64;
        let input = b"rtk";
        let invalid = [255_u8];
        for (abi, address, length, capacity, expected) in [
            (2, input.as_ptr() as u64, 3, 512, 4),
            (1, input.as_ptr() as u64, 3, 0, 2),
            (1, invalid.as_ptr() as u64, 1, 512, 1),
            (1, 0, 3, 512, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_mojo_super_expose_optional_alias_plan_v1(
                        abi,
                        address,
                        length,
                        output.as_mut_ptr() as u64,
                        capacity,
                        (&mut written as *mut i64) as u64,
                    )
                },
                expected
            );
            assert_eq!(written, 71);
            assert_eq!(output, [83; 512]);
        }
    }
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
