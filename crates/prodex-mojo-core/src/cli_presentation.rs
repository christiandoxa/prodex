//! Compiled CLI help and process presentation. Rust owns Clap tags and text storage.
use crate::{MojoError, text_output::render_measured_text};
use std::sync::OnceLock;

/// Stable help slots in the version-1 CLI presentation ABI.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum CliHelpKind {
    TopLevel = 0,
    Profile = 1,
    Login = 2,
    Quota = 3,
    Redeem = 4,
    Run = 5,
    Super = 6,
    Doctor = 7,
    Session = 8,
}

/// Rust-only Clap variants cross the ABI as these explicit representation tags.
#[derive(Debug, Clone, Copy)]
#[repr(i64)]
pub enum CliCommandKind {
    Profile = 0,
    UseProfile = 1,
    Current = 2,
    Info = 3,
    Status = 4,
    Log = 5,
    Session = 6,
    Doctor = 7,
    Login = 8,
    Logout = 9,
    Update = 10,
    Quota = 11,
    Redeem = 12,
    Ping = 13,
    Run = 14,
    Super = 15,
    Gateway = 16,
    SuperExpose = 17,
    RuntimeBroker = 18,
    McpJsonlBridge = 19,
    SubAgentExec = 20,
}

#[derive(Clone, Copy)]
#[repr(i64)]
enum TextKind {
    Help = 0,
    CommandLabel = 1,
}

unsafe extern "C" {
    fn prodex_cli_presentation_text_v1(
        abi: i64,
        operation: i64,
        kind: i64,
        measuring: i64,
        output_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_cli_command_runtime_v1(abi: i64, kind: i64) -> i64;
}

fn load_text(operation: TextKind, kind: i64) -> Result<String, MojoError> {
    render_measured_text(|measuring, output, capacity, written| unsafe {
        prodex_cli_presentation_text_v1(
            1,
            operation as i64,
            kind,
            measuring,
            output,
            capacity,
            written,
        )
    })
}

/// Load one compiled help body into caller-owned storage for Clap.
pub fn help_text(kind: CliHelpKind) -> Result<String, MojoError> {
    load_text(TextKind::Help, kind as i64)
}

/// Preserve static public labels using Rust-owned cached compiled text.
pub fn process_label(kind: CliCommandKind) -> Result<&'static str, MojoError> {
    static LABELS: OnceLock<Result<Vec<String>, MojoError>> = OnceLock::new();
    match LABELS.get_or_init(|| {
        (0..=CliCommandKind::SubAgentExec as i64)
            .map(|kind| load_text(TextKind::CommandLabel, kind))
            .collect()
    }) {
        Ok(labels) => labels
            .get(kind as usize)
            .map(String::as_str)
            .ok_or(MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

/// Mojo owns which canonical commands count as runtime processes.
pub fn launches_runtime(kind: CliCommandKind) -> Result<bool, MojoError> {
    match unsafe { prodex_cli_command_runtime_v1(1, kind as i64) } {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        -2 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_cli_presentation_preserves_all_help_bytes_and_command_metadata() {
        for (kind, expected_length, expected_hash) in [
            (CliHelpKind::TopLevel, 426, 0x82460174480663e0_u64),
            (CliHelpKind::Profile, 827, 0x542f61f5b923e4ff_u64),
            (CliHelpKind::Login, 724, 0xd5f3c28c01183558_u64),
            (CliHelpKind::Quota, 1239, 0x681bf5372c6943af_u64),
            (CliHelpKind::Redeem, 511, 0xb3a64c323850321a_u64),
            (CliHelpKind::Run, 980, 0x248df9759a8e94d3_u64),
            (CliHelpKind::Super, 4483, 0x9901b75cb564c7d0_u64),
            (CliHelpKind::Doctor, 230, 0xfe4dbd8f55073010_u64),
            (CliHelpKind::Session, 355, 0xc11a6d0aafde817a_u64),
        ] {
            let text = help_text(kind).unwrap();
            let hash = text.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
            assert_eq!((text.len(), hash), (expected_length, expected_hash));
        }
        for (kind, label, runtime) in [
            (CliCommandKind::Profile, "profile", false),
            (CliCommandKind::UseProfile, "use", false),
            (CliCommandKind::Current, "current", false),
            (CliCommandKind::Info, "info", false),
            (CliCommandKind::Status, "status", false),
            (CliCommandKind::Log, "log", false),
            (CliCommandKind::Session, "session", false),
            (CliCommandKind::Doctor, "doctor", false),
            (CliCommandKind::Login, "login", false),
            (CliCommandKind::Logout, "logout", false),
            (CliCommandKind::Update, "update", false),
            (CliCommandKind::Quota, "quota", false),
            (CliCommandKind::Redeem, "redeem", false),
            (CliCommandKind::Ping, "ping", false),
            (CliCommandKind::Run, "run", true),
            (CliCommandKind::Super, "super", true),
            (CliCommandKind::Gateway, "gateway", true),
            (CliCommandKind::SuperExpose, "super-expose", true),
            (CliCommandKind::RuntimeBroker, "__runtime-broker", true),
            (CliCommandKind::McpJsonlBridge, "__mcp-jsonl-bridge", false),
            (CliCommandKind::SubAgentExec, "__sub-agent-exec", false),
        ] {
            assert_eq!(process_label(kind).unwrap(), label);
            assert_eq!(launches_runtime(kind), Ok(runtime));
        }
    }

    #[test]
    fn compiled_cli_presentation_abi_rejects_invalid_fields_before_output() {
        let mut output = [93_u8; 32];
        let mut written = 77_i64;
        let target = output.as_mut_ptr() as u64;
        let length = (&mut written as *mut i64) as u64;
        for (version, operation, kind, measuring, destination, capacity, count, expected) in [
            (2, 0, 0, 0, target, 32, length, 4),
            (1, 2, 0, 0, target, 32, length, 1),
            (1, 0, 9, 0, target, 32, length, 1),
            (1, 1, 21, 0, target, 32, length, 1),
            (1, 1, -1, 0, target, 32, length, 1),
            (1, 1, 0, 2, target, 32, length, 1),
            (1, 1, 0, 1, target, 0, length, 1),
            (1, 1, 0, 0, 0, 32, length, 1),
            (1, 1, 0, 0, target, 32, length + 1, 1),
            (1, 1, 0, 0, target, 1, length, 2),
        ] {
            assert_eq!(
                unsafe {
                    prodex_cli_presentation_text_v1(
                        version,
                        operation,
                        kind,
                        measuring,
                        destination,
                        capacity,
                        count,
                    )
                },
                expected
            );
            assert_eq!(output, [93; 32]);
            assert_eq!(written, 77);
        }
        assert_eq!(unsafe { prodex_cli_command_runtime_v1(2, 0) }, -2);
        assert_eq!(unsafe { prodex_cli_command_runtime_v1(1, -1) }, -1);
        assert_eq!(unsafe { prodex_cli_command_runtime_v1(1, 21) }, -1);
    }
}
