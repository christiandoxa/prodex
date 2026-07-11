use std::collections::BTreeMap;

use crate::RuntimeDoctorSummary;

use super::super::marker_accessors::runtime_doctor_marker_count;

pub(super) fn runtime_doctor_compact_exit_counts(
    summary: &RuntimeDoctorSummary,
) -> BTreeMap<String, usize> {
    let compact_markers: [(&str, &[&str]); 11] = [
        (
            "candidate_exhausted",
            &[
                "compact_candidate_exhausted",
                "compact_exit_candidate_exhausted",
            ],
        ),
        (
            "committed",
            &["compact_committed", "compact_exit_committed"],
        ),
        (
            "committed_owner",
            &["compact_committed_owner", "compact_exit_committed_owner"],
        ),
        (
            "followup_owner",
            &["compact_followup_owner", "compact_exit_followup_owner"],
        ),
        (
            "lineage_released",
            &["compact_lineage_released", "compact_exit_lineage_released"],
        ),
        (
            "owner_retry",
            &[
                "compact_overload_conservative_retry",
                "compact_exit_overload_conservative_retry",
            ],
        ),
        (
            "precommit_budget",
            &[
                "compact_precommit_budget_exhausted",
                "compact_exit_precommit_budget_exhausted",
            ],
        ),
        (
            "pressure_shed",
            &["compact_pressure_shed", "compact_exit_pressure_shed"],
        ),
        (
            "quota_misc",
            &[
                "compact_quota_unclassified",
                "compact_exit_quota_unclassified",
            ],
        ),
        (
            "retryable_failure",
            &[
                "compact_retryable_failure",
                "compact_exit_retryable_failure",
            ],
        ),
        ("transport_failure", &["compact_transport_failure"]),
    ];

    compact_markers
        .into_iter()
        .filter_map(|(label, markers)| {
            let count = markers
                .iter()
                .map(|marker| runtime_doctor_marker_count(summary, marker))
                .sum::<usize>();
            (count > 0).then(|| (label.to_string(), count))
        })
        .collect()
}
