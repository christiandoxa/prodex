use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};
use crate::MojoError;

const RUNTIME_DOCTOR_MARKER_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_marker_known_v1(abi_version: i64, marker: u64, known: u64)
    -> i64;
    fn prodex_mojo_runtime_doctor_marker_semantics_v1(
        abi_version: i64,
        marker: u64,
        output: u64,
    ) -> i64;
}

pub const RUNTIME_DOCTOR_MARKER_PHASE_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_MARKER_PHASE_SELECTION: i64 = 1;
pub const RUNTIME_DOCTOR_MARKER_PHASE_PRE_SEND: i64 = 2;
pub const RUNTIME_DOCTOR_MARKER_PHASE_UPSTREAM: i64 = 3;
pub const RUNTIME_DOCTOR_MARKER_PHASE_COMMIT: i64 = 4;
pub const RUNTIME_DOCTOR_MARKER_PHASE_FAIL: i64 = 5;

pub const RUNTIME_DOCTOR_MARKER_SELECTION_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_MARKER_SELECTION_PICKED: i64 = 1;
pub const RUNTIME_DOCTOR_MARKER_SELECTION_KEPT: i64 = 2;
pub const RUNTIME_DOCTOR_MARKER_SELECTION_SKIPPED: i64 = 3;
pub const RUNTIME_DOCTOR_MARKER_SELECTION_BLOCKED: i64 = 4;

pub const RUNTIME_DOCTOR_MARKER_ROUTE_NONE: i64 = 0;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_SELECTED: i64 = 1;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_SELECTION_SKIP: i64 = 2;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_BLOCKED: i64 = 3;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_HEALTH: i64 = 4;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_HEALTH: i64 = 5;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_TRANSPORT_FAILURE: i64 = 6;
pub const RUNTIME_DOCTOR_MARKER_ROUTE_QUOTA: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorMarkerSemantics {
    pub timeline_phase: i64,
    pub selection_bucket: i64,
    pub route_action: i64,
}

pub fn runtime_doctor_marker_semantics(
    marker: &str,
) -> Result<RuntimeDoctorMarkerSemantics, MojoError> {
    ensure_rich_abi()?;
    let marker = view(marker);
    let mut output = [0_i64; 3];
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_semantics_v1(
            RUNTIME_DOCTOR_MARKER_ABI_VERSION,
            mojo_pointer_address(&marker),
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            _ => MojoError::InvalidOutput,
        });
    }
    if !(0..=RUNTIME_DOCTOR_MARKER_PHASE_FAIL).contains(&output[0])
        || !(0..=RUNTIME_DOCTOR_MARKER_SELECTION_BLOCKED).contains(&output[1])
        || !(0..=RUNTIME_DOCTOR_MARKER_ROUTE_QUOTA).contains(&output[2])
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RuntimeDoctorMarkerSemantics {
        timeline_phase: output[0],
        selection_bucket: output[1],
        route_action: output[2],
    })
}

pub fn runtime_doctor_marker_known(marker: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let marker = view(marker);
    let mut known = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_known_v1(
            RUNTIME_DOCTOR_MARKER_ABI_VERSION,
            mojo_pointer_address(&marker),
            mojo_mut_pointer_address(&mut known),
        )
    };
    match (status, known) {
        (0, 0) => Ok(false),
        (0, 1) => Ok(true),
        (1, _) => Err(MojoError::InvalidInput),
        (2, _) => Err(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_doctor_marker_classifier_accepts_known_and_rejects_unknown() {
        assert!(runtime_doctor_marker_known("selection_pick").unwrap());
        assert!(runtime_doctor_marker_known("websocket_connect_overflow_rejected").unwrap());
        assert!(!runtime_doctor_marker_known("not_a_runtime_marker").unwrap());
    }
}
