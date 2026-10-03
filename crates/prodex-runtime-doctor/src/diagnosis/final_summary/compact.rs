use std::collections::BTreeMap;

use crate::RuntimeDoctorSummary;

pub(super) fn runtime_doctor_compact_exit_counts(
    summary: &RuntimeDoctorSummary,
) -> BTreeMap<String, usize> {
    prodex_mojo_core::rich::runtime_doctor_compact_exit_counts(
        summary
            .marker_counts
            .iter()
            .map(|(marker, count)| (marker.as_str(), *count)),
    )
    .expect("Mojo runtime-doctor compact-exit reducer returned invalid output")
    .into_iter()
    .collect()
}
