//! Mojo-backed RTK shell classification and insertion.

use crate::MojoError;

const RTK_NOISY_ABI_VERSION: i64 = 1;
const RTK_NOISY_MODE_WRAPPED: i64 = 0;
const RTK_NOISY_MODE_PREFIXED: i64 = 1;
const RTK_NOISY_STATUS_CAPACITY: i64 = 3;

unsafe extern "C" {
    fn prodex_mojo_rtk_noisy_shell_command_v1(
        abi_version: i64,
        mode: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::InvalidInput),
        RTK_NOISY_STATUS_CAPACITY => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn rewrite(command: &str, mode: i64) -> Result<Option<String>, MojoError> {
    if command.len() > 4 * 1024 * 1024 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = vec![
        0_u8;
        command
            .len()
            .checked_add(4)
            .ok_or(MojoError::InvalidInput)?
    ];
    let mut written = 0_i64;
    status(unsafe {
        prodex_mojo_rtk_noisy_shell_command_v1(
            RTK_NOISY_ABI_VERSION,
            mode,
            command.as_ptr() as usize as u64,
            i64::try_from(command.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
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

/// Inserts `rtk` at the first noisy command segment.
pub fn wrapped_shell_command(command: &str) -> Result<Option<String>, MojoError> {
    rewrite(command, RTK_NOISY_MODE_WRAPPED)
}

/// Prefixes the complete command when any segment is noisy.
pub fn prefixed_noisy_shell_command(command: &str) -> Result<Option<String>, MojoError> {
    rewrite(command, RTK_NOISY_MODE_PREFIXED)
}

#[cfg(test)]
mod tests {
    use super::{prefixed_noisy_shell_command, wrapped_shell_command};

    #[test]
    fn mojo_rtk_parser_handles_shell_boundaries_and_quotes() {
        assert_eq!(
            wrapped_shell_command("printf 'a; b' && cargo test").unwrap(),
            Some("printf 'a; b' && rtk cargo test".to_string())
        );
        assert_eq!(wrapped_shell_command("git 'cargo test'").unwrap(), None);
    }

    #[test]
    fn mojo_rtk_parser_handles_assignments_unicode_space_and_subcommands() {
        assert_eq!(
            wrapped_shell_command("RUST_LOG=é\u{3000}cargo\u{3000}test").unwrap(),
            Some("RUST_LOG=é\u{3000}rtk cargo\u{3000}test".to_string())
        );
        assert_eq!(wrapped_shell_command("cargo fmt").unwrap(), None);
        assert_eq!(
            prefixed_noisy_shell_command("cd /repo && cargo check").unwrap(),
            Some("rtk cd /repo && cargo check".to_string())
        );
    }
}
