//! Versioned selection-record projection; Mojo owns all semantic decisions.
use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_selection_record_plan_v1(
        abi_version: i64,
        marker: u64,
        field_presence_bits: i64,
        output: u64,
        output_capacity: i64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDoctorSelectionDestination {
    Selected,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDoctorSelectionReason {
    Field(usize),
    Marker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorSelectionRecordPlan {
    pub destination: RuntimeDoctorSelectionDestination,
    pub reason: Option<RuntimeDoctorSelectionReason>,
}

/// Mojo owns selection counting and rejection-reason precedence. Rust only
/// supplies field presence (including empty values) and materializes maps.
pub fn runtime_doctor_selection_record_plan(
    marker: &str,
    reason_fields: [Option<&str>; 4],
) -> Result<Option<RuntimeDoctorSelectionRecordPlan>, MojoError> {
    ensure_rich_abi()?;
    let marker = view(marker);
    let presence = reason_fields
        .iter()
        .enumerate()
        .fold(0_i64, |bits, (index, field)| {
            bits | (i64::from(field.is_some()) << index)
        });
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_mojo_runtime_doctor_selection_record_plan_v1(
            1,
            mojo_pointer_address(&marker),
            presence,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            3,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output {
        [0, 0, 0] => Ok(None),
        [1 | 2, 1, 0] => Ok(Some(RuntimeDoctorSelectionRecordPlan {
            destination: RuntimeDoctorSelectionDestination::Selected,
            reason: None,
        })),
        [3 | 4, 2, 1..=4] => Ok(Some(RuntimeDoctorSelectionRecordPlan {
            destination: RuntimeDoctorSelectionDestination::Rejected,
            reason: Some(RuntimeDoctorSelectionReason::Field(
                usize::try_from(output[2] - 1).map_err(|_| MojoError::InvalidOutput)?,
            )),
        })),
        [3 | 4, 2, 5] => Ok(Some(RuntimeDoctorSelectionRecordPlan {
            destination: RuntimeDoctorSelectionDestination::Rejected,
            reason: Some(RuntimeDoctorSelectionReason::Marker),
        })),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_record_plan_checks_marker_and_reason_source_precedence() {
        use RuntimeDoctorSelectionDestination::{Rejected, Selected};
        use RuntimeDoctorSelectionReason::{Field, Marker};

        assert_eq!(
            runtime_doctor_selection_record_plan("selection_pick", [None; 4]).unwrap(),
            Some(RuntimeDoctorSelectionRecordPlan {
                destination: Selected,
                reason: None,
            }),
        );
        assert_eq!(
            runtime_doctor_selection_record_plan(
                "selection_skip_affinity",
                [Some(""), Some("outcome"), None, None]
            )
            .unwrap(),
            Some(RuntimeDoctorSelectionRecordPlan {
                destination: Rejected,
                reason: Some(Field(0)),
            }),
        );
        assert_eq!(
            runtime_doctor_selection_record_plan(
                "local_selection_blocked",
                [None, Some("outcome"), Some("event"), None]
            )
            .unwrap(),
            Some(RuntimeDoctorSelectionRecordPlan {
                destination: Rejected,
                reason: Some(Field(1)),
            }),
        );
        assert_eq!(
            runtime_doctor_selection_record_plan("compact_candidate_exhausted", [None; 4]).unwrap(),
            Some(RuntimeDoctorSelectionRecordPlan {
                destination: Rejected,
                reason: Some(Marker),
            }),
        );
        assert_eq!(
            runtime_doctor_selection_record_plan("first_upstream_chunk", [Some("ignored"); 4])
                .unwrap(),
            None,
        );
        let marker = view("selection_pick");
        let mut result = [-1_i64; 3];
        unsafe {
            assert_eq!(
                prodex_mojo_runtime_doctor_selection_record_plan_v1(
                    2,
                    mojo_pointer_address(&marker),
                    0,
                    mojo_mut_pointer_address(result.as_mut_ptr()),
                    3,
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_selection_record_plan_v1(
                    1,
                    mojo_pointer_address(&marker),
                    16,
                    mojo_mut_pointer_address(result.as_mut_ptr()),
                    3,
                ),
                1,
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_selection_record_plan_v1(
                    1,
                    mojo_pointer_address(&marker),
                    0,
                    mojo_mut_pointer_address(result.as_mut_ptr()),
                    2,
                ),
                1,
            );
        }
        assert_eq!(result, [-1_i64; 3]);
    }
}
