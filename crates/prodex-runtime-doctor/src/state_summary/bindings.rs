use std::collections::{BTreeMap, BTreeSet};

use crate::{
    RuntimeDoctorBindingProfileSummary, RuntimeDoctorBindingSourceSummary,
    RuntimeDoctorBindingStateSummary,
};

const RUNTIME_DOCTOR_BINDING_SAMPLE_LIMIT: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorBindingSourceInput<'a, B> {
    pub response_profile_bindings: &'a BTreeMap<String, B>,
    pub session_profile_bindings: &'a BTreeMap<String, B>,
    pub turn_state_bindings: &'a BTreeMap<String, B>,
    pub session_id_bindings: &'a BTreeMap<String, B>,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorBindingStateInput<'a, B> {
    pub active_profile: Option<&'a str>,
    pub profile_names: &'a [String],
    pub last_run_selected_profiles: usize,
    pub state: RuntimeDoctorBindingSourceInput<'a, B>,
    pub runtime_continuations: RuntimeDoctorBindingSourceInput<'a, B>,
    pub continuation_journal: RuntimeDoctorBindingSourceInput<'a, B>,
    pub merged_continuations: RuntimeDoctorBindingSourceInput<'a, B>,
}

fn runtime_doctor_record_binding_source_kind<B, F, G>(
    source: &mut RuntimeDoctorBindingSourceSummary,
    profile_counts: &mut BTreeMap<String, RuntimeDoctorBindingProfileSummary>,
    known_profiles: &BTreeSet<&str>,
    kind: &str,
    bindings: &BTreeMap<String, B>,
    profile_name: F,
    bound_at: G,
) where
    F: Fn(&B) -> &str + Copy,
    G: Fn(&B) -> i64 + Copy,
{
    for (key, binding) in bindings {
        let profile = profile_name(binding);
        let bound_at = bound_at(binding);
        source.oldest_bound_at = Some(
            source
                .oldest_bound_at
                .map(|oldest| oldest.min(bound_at))
                .unwrap_or(bound_at),
        );
        source.newest_bound_at = Some(
            source
                .newest_bound_at
                .map(|newest| newest.max(bound_at))
                .unwrap_or(bound_at),
        );
        if !known_profiles.contains(profile) {
            source.missing_profile_bindings += 1;
            if source.missing_profile_binding_samples.len() < RUNTIME_DOCTOR_BINDING_SAMPLE_LIMIT {
                source
                    .missing_profile_binding_samples
                    .push(format!("{kind}:{key}->{profile}"));
            }
        }
        let profile_entry = profile_counts
            .entry(profile.to_string())
            .or_insert_with(|| RuntimeDoctorBindingProfileSummary {
                profile: profile.to_string(),
                ..RuntimeDoctorBindingProfileSummary::default()
            });
        match kind {
            "response" => profile_entry.response_bindings += 1,
            "session" => profile_entry.session_bindings += 1,
            "turn_state" => profile_entry.turn_state_bindings += 1,
            "session_id" => profile_entry.session_id_bindings += 1,
            _ => {}
        }
        profile_entry.total_bindings += 1;
    }
}

fn runtime_doctor_binding_source_summary<B, F, G>(
    input: RuntimeDoctorBindingSourceInput<'_, B>,
    known_profiles: &BTreeSet<&str>,
    profile_name: F,
    bound_at: G,
) -> RuntimeDoctorBindingSourceSummary
where
    F: Fn(&B) -> &str + Copy,
    G: Fn(&B) -> i64 + Copy,
{
    let mut source = RuntimeDoctorBindingSourceSummary {
        response_bindings: input.response_profile_bindings.len(),
        session_bindings: input.session_profile_bindings.len(),
        turn_state_bindings: input.turn_state_bindings.len(),
        session_id_bindings: input.session_id_bindings.len(),
        total_bindings: input.response_profile_bindings.len()
            + input.session_profile_bindings.len()
            + input.turn_state_bindings.len()
            + input.session_id_bindings.len(),
        ..RuntimeDoctorBindingSourceSummary::default()
    };
    let mut profile_counts = BTreeMap::new();
    runtime_doctor_record_binding_source_kind(
        &mut source,
        &mut profile_counts,
        known_profiles,
        "response",
        input.response_profile_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_record_binding_source_kind(
        &mut source,
        &mut profile_counts,
        known_profiles,
        "session",
        input.session_profile_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_record_binding_source_kind(
        &mut source,
        &mut profile_counts,
        known_profiles,
        "turn_state",
        input.turn_state_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_record_binding_source_kind(
        &mut source,
        &mut profile_counts,
        known_profiles,
        "session_id",
        input.session_id_bindings,
        profile_name,
        bound_at,
    );
    source.profiles = profile_counts.into_values().collect();
    source.profiles.sort_by(|left, right| {
        right
            .total_bindings
            .cmp(&left.total_bindings)
            .then_with(|| left.profile.cmp(&right.profile))
    });
    source.profile_count = source.profiles.len();
    source
}

pub fn runtime_doctor_binding_state_summary<B, F, G>(
    input: RuntimeDoctorBindingStateInput<'_, B>,
    profile_name: F,
    bound_at: G,
) -> RuntimeDoctorBindingStateSummary
where
    F: Fn(&B) -> &str + Copy,
    G: Fn(&B) -> i64 + Copy,
{
    let known_profiles = input
        .profile_names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    RuntimeDoctorBindingStateSummary {
        active_profile: input.active_profile.map(str::to_string),
        profile_count: input.profile_names.len(),
        last_run_selected_profiles: input.last_run_selected_profiles,
        state: runtime_doctor_binding_source_summary(
            input.state,
            &known_profiles,
            profile_name,
            bound_at,
        ),
        runtime_continuations: runtime_doctor_binding_source_summary(
            input.runtime_continuations,
            &known_profiles,
            profile_name,
            bound_at,
        ),
        continuation_journal: runtime_doctor_binding_source_summary(
            input.continuation_journal,
            &known_profiles,
            profile_name,
            bound_at,
        ),
        merged_continuations: runtime_doctor_binding_source_summary(
            input.merged_continuations,
            &known_profiles,
            profile_name,
            bound_at,
        ),
    }
}
