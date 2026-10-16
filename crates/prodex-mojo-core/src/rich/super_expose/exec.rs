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

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
