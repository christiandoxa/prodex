//! Ping diagnostic plans after native error/process acquisition and secret redaction.
use super::{
    PING_PROTOCOL_ABI_VERSION, PING_STATUS_CANCELLED, PING_STATUS_DETAIL_MAX_BYTES, ensure_rich_abi,
};
use crate::MojoError;

/// The existing output-byte limit, also used to allocate native ABI storage.
pub const PING_ERROR_DETAIL_MAX_BYTES: usize = 4096;

/// Native child status acquired without moving process APIs across the ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PingChildExit {
    Success,
    Failed(i32),
}

#[repr(i64)]
#[derive(Clone, Copy)]
enum DiagnosticKind {
    Bounded,
    Process,
    Output,
}

unsafe extern "C" {
    fn prodex_mojo_ping_diagnostic_v1(
        version: i64,
        operation: i64,
        raw_address: u64,
        raw_length: i64,
        redacted_address: u64,
        redacted_length: i64,
        exit_failed: i64,
        exit_code: i64,
        output_address: u64,
        output_capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

fn diagnostic(
    kind: DiagnosticKind,
    raw: &str,
    redacted: &str,
    exit: PingChildExit,
) -> Result<(i64, String), MojoError> {
    ensure_rich_abi()?;
    let label_bytes = match kind {
        DiagnosticKind::Output => raw.len(),
        DiagnosticKind::Bounded | DiagnosticKind::Process => 0,
    };
    let capacity = PING_ERROR_DETAIL_MAX_BYTES
        .checked_add(PING_STATUS_DETAIL_MAX_BYTES + 64)
        .and_then(|capacity| capacity.checked_add(label_bytes))
        .ok_or(MojoError::Capacity)?;
    let mut output = vec![0_u8; capacity];
    let mut metadata = [-2_i64, -1];
    let (exit_failed, exit_code) = match exit {
        PingChildExit::Success => (0, 0),
        PingChildExit::Failed(code) => (1, i64::from(code)),
    };
    match unsafe {
        prodex_mojo_ping_diagnostic_v1(
            PING_PROTOCOL_ABI_VERSION,
            kind as i64,
            raw.as_ptr() as u64,
            i64::try_from(raw.len()).map_err(|_| MojoError::InvalidInput)?,
            redacted.as_ptr() as u64,
            i64::try_from(redacted.len()).map_err(|_| MojoError::InvalidInput)?,
            exit_failed,
            exit_code,
            output.as_mut_ptr() as u64,
            i64::try_from(capacity).map_err(|_| MojoError::Capacity)?,
            metadata.as_mut_ptr() as u64,
        )
    } {
        0 => {}
        1 | 2 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let valid_status = match kind {
        DiagnosticKind::Process => (0..=PING_STATUS_CANCELLED).contains(&metadata[0]),
        DiagnosticKind::Bounded | DiagnosticKind::Output => metadata[0] == -1,
    };
    let written = usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)?;
    if !valid_status || written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    Ok((
        metadata[0],
        String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)?,
    ))
}

/// The caller redacts secrets before Mojo normalizes and bounds diagnostic text.
pub fn ping_bounded_detail(redacted: &str) -> Result<String, MojoError> {
    diagnostic(
        DiagnosticKind::Bounded,
        "",
        redacted,
        PingChildExit::Success,
    )
    .map(|(_, text)| text)
}

/// Raw errors are classification-only; output contains only labels and redacted text.
pub fn ping_process_error(raw: &str, redacted: &str) -> Result<(i64, String), MojoError> {
    diagnostic(
        DiagnosticKind::Process,
        raw,
        redacted,
        PingChildExit::Success,
    )
}

/// Compose a native exit status and redacted stderr with the existing base detail.
pub fn ping_output_failure_detail(
    base: &str,
    exit: PingChildExit,
    redacted_stderr: &str,
) -> Result<String, MojoError> {
    diagnostic(DiagnosticKind::Output, base, redacted_stderr, exit).map(|(_, text)| text)
}

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod tests;
