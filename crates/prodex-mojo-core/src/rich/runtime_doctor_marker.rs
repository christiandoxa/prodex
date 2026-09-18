use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};
use crate::MojoError;

const RUNTIME_DOCTOR_MARKER_ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_marker_known_v1(abi_version: i64, marker: u64, known: u64)
    -> i64;
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
