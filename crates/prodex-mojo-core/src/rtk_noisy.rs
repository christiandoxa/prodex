//! Mojo-backed RTK shell classification and insertion.

use crate::MojoError;

const RTK_NOISY_ABI_VERSION: i64 = 1;
const RTK_NOISY_MODE_WRAPPED: i64 = 0;
const RTK_NOISY_MODE_PREFIXED: i64 = 1;
const RTK_NOISY_STATUS_CAPACITY: i64 = 3;

unsafe extern "C" {
    fn prodex_mojo_rtk_wrapper_manifest_v1(
        abi_version: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
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
    if command.len() > 64 * 1024 * 1024 {
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

/// One compiled owner for automatic RTK wrapper command/subcommand policy.
/// The host parses the Mojo-provided ASCII manifest solely to perform
/// discovery and emit shell-script wrappers, never a Rust command table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtkAutoWrapperPolicy {
    pub command: String,
    pub subcommands: Vec<String>,
}

pub fn rtk_auto_wrapper_manifest() -> Result<Vec<RtkAutoWrapperPolicy>, MojoError> {
    let mut output = vec![0_u8; 2048];
    let mut written = -1_i64;
    status(unsafe {
        prodex_mojo_rtk_wrapper_manifest_v1(
            RTK_NOISY_ABI_VERSION,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    let text = std::str::from_utf8(&output[..written]).map_err(|_| MojoError::InvalidOutput)?;
    let mut result = Vec::new();
    for line in text.lines() {
        let (command, raw_subcommands) = line.split_once('|').ok_or(MojoError::InvalidOutput)?;
        if command.is_empty()
            || !command.bytes().all(|c| c.is_ascii_lowercase() || c == b'-')
            || result
                .iter()
                .any(|entry: &RtkAutoWrapperPolicy| entry.command == command)
        {
            return Err(MojoError::InvalidOutput);
        }
        let subcommands = if raw_subcommands.is_empty() {
            vec![]
        } else {
            raw_subcommands
                .split(',')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        if subcommands.iter().any(|subcommand| {
            subcommand.is_empty()
                || !subcommand
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c == b'-')
        }) {
            return Err(MojoError::InvalidOutput);
        }
        result.push(RtkAutoWrapperPolicy {
            command: command.to_owned(),
            subcommands,
        });
    }
    if result.is_empty() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(result)
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
    use super::{
        prefixed_noisy_shell_command, prodex_mojo_rtk_wrapper_manifest_v1,
        rtk_auto_wrapper_manifest, wrapped_shell_command,
    };

    #[test]
    fn compiled_rtk_wrapper_manifest_preserves_command_order_and_exact_subcommands() {
        let manifest = rtk_auto_wrapper_manifest().unwrap();
        assert_eq!(manifest.len(), 14);
        assert_eq!(
            manifest
                .iter()
                .map(|item| item.command.as_str())
                .collect::<Vec<_>>(),
            [
                "git", "cargo", "npm", "yarn", "pnpm", "bun", "pytest", "go", "docker", "kubectl",
                "rg", "find", "ls", "tree"
            ],
        );
        assert_eq!(
            manifest[0].subcommands,
            ["diff", "show", "log", "status", "grep", "blame"]
        );
        assert_eq!(
            manifest[1].subcommands,
            ["test", "build", "check", "clippy", "bench", "run"]
        );
        assert_eq!(
            manifest[2].subcommands,
            ["test", "run", "build", "install", "ci", "update", "audit"]
        );
        assert_eq!(
            manifest[3].subcommands,
            ["test", "run", "build", "install", "add", "upgrade"]
        );
        assert_eq!(
            manifest[4].subcommands,
            ["test", "run", "build", "install", "add", "update"]
        );
        assert_eq!(
            manifest[5].subcommands,
            ["test", "run", "build", "install", "add"]
        );
        assert!(manifest[6].subcommands.is_empty());
        assert_eq!(manifest[7].subcommands, ["test", "build", "vet"]);
        assert_eq!(
            manifest[8].subcommands,
            ["build", "compose", "logs", "pull", "push", "run"]
        );
        assert_eq!(
            manifest[9].subcommands,
            ["logs", "describe", "get", "events", "top"]
        );
        for entry in &manifest[10..] {
            assert!(entry.subcommands.is_empty());
        }
    }

    #[test]
    fn compiled_rtk_wrapper_manifest_abi_validates_capacity_version_and_null_without_writes() {
        let mut output = [77_u8; 1024];
        let mut written = 91_i64;
        let out = output.as_mut_ptr() as usize as u64;
        let ptr = (&mut written as *mut i64) as usize as u64;
        let call = |version, dest, capacity, count| unsafe {
            prodex_mojo_rtk_wrapper_manifest_v1(version, dest, capacity, count)
        };
        assert_eq!(call(2, out, 1024, ptr), 4);
        assert_eq!(call(1, 0, 1024, ptr), 1);
        assert_eq!(call(1, out, -1, ptr), 1);
        assert_eq!(call(1, out, 1024, 0), 1);
        assert_eq!(call(1, out, 1, ptr), 3);
        assert_eq!(output, [77; 1024]);
        assert_eq!(written, 91);
    }

    #[test]
    fn mojo_rtk_parser_handles_shell_boundaries_and_quotes() {
        assert_eq!(
            wrapped_shell_command("printf 'a; b' && cargo test").unwrap(),
            Some("printf 'a; b' && rtk cargo test".to_string())
        );
        assert_eq!(wrapped_shell_command("git 'cargo test'").unwrap(), None);
    }

    #[test]
    fn mojo_rtk_parser_preserves_large_shell_command_without_abi_failure() {
        let command = format!("rg {}", "x".repeat(5 * 1024 * 1024));
        let rewritten = wrapped_shell_command(&command).unwrap().unwrap();
        assert_eq!(rewritten, format!("rtk {command}"));
    }

    #[test]
    fn mojo_rtk_parser_skips_oversized_environment_values_before_noisy_command() {
        let command = format!("CACHE={} cargo test", "z".repeat(5000));
        let rewritten = wrapped_shell_command(&command).unwrap().unwrap();
        assert_eq!(rewritten, command.replace(" cargo test", " rtk cargo test"));
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
