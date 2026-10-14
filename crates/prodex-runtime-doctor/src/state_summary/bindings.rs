use std::collections::{BTreeMap, BTreeSet};

use crate::{
    RuntimeDoctorBindingProfileSummary, RuntimeDoctorBindingSourceSummary,
    RuntimeDoctorBindingStateSummary,
};

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

struct RuntimeDoctorBindingSourceEvent<'a> {
    kind: i64,
    label: &'static str,
    key: &'a str,
    profile: &'a str,
    bound_at: i64,
}

fn runtime_doctor_collect_binding_kind<'a, B, F, G>(
    events: &mut Vec<RuntimeDoctorBindingSourceEvent<'a>>,
    kind: i64,
    label: &'static str,
    bindings: &'a BTreeMap<String, B>,
    profile_name: F,
    bound_at: G,
) where
    F: Fn(&B) -> &str + Copy,
    G: Fn(&B) -> i64 + Copy,
{
    for (key, binding) in bindings {
        events.push(RuntimeDoctorBindingSourceEvent {
            kind,
            label,
            key,
            profile: profile_name(binding),
            bound_at: bound_at(binding),
        });
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
    use prodex_mojo_core::rich::RuntimeDoctorBindingRow;

    let mut events = Vec::new();
    runtime_doctor_collect_binding_kind(
        &mut events,
        0,
        "response",
        input.response_profile_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_collect_binding_kind(
        &mut events,
        1,
        "session",
        input.session_profile_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_collect_binding_kind(
        &mut events,
        2,
        "turn_state",
        input.turn_state_bindings,
        profile_name,
        bound_at,
    );
    runtime_doctor_collect_binding_kind(
        &mut events,
        3,
        "session_id",
        input.session_id_bindings,
        profile_name,
        bound_at,
    );
    // Rust preserves borrowed names and known-profile lookup; Mojo computes
    // counts, chronology, sample admission and the ranked profile permutation.
    let mut indices = BTreeMap::<&str, usize>::new();
    for event in &events {
        indices.entry(event.profile).or_default();
    }
    for (index, value) in indices.values_mut().enumerate() {
        *value = index;
    }
    let names = indices.keys().copied().collect::<Vec<_>>();
    let rows = events
        .iter()
        .map(|event| RuntimeDoctorBindingRow {
            profile_index: *indices
                .get(event.profile)
                .expect("every scanned binding profile has a borrowed name index"),
            kind: event.kind,
            bound_at: event.bound_at,
            known_profile: known_profiles.contains(event.profile),
        })
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::rich::runtime_doctor_binding_source_plan(&rows, names.len())
        .expect("Mojo runtime-doctor binding reducer returned invalid output");
    let missing_profile_binding_samples = plan
        .missing_sample_indices
        .into_iter()
        .map(|index| {
            let event = &events[index];
            format!("{}:{}->{}", event.label, event.key, event.profile)
        })
        .collect();
    let profiles: Vec<_> = plan
        .profiles
        .into_iter()
        .map(|profile| RuntimeDoctorBindingProfileSummary {
            profile: names[profile.name_index].to_string(),
            response_bindings: profile.counts[0],
            session_bindings: profile.counts[1],
            turn_state_bindings: profile.counts[2],
            session_id_bindings: profile.counts[3],
            total_bindings: profile.total,
        })
        .collect();
    RuntimeDoctorBindingSourceSummary {
        response_bindings: plan.by_kind[0],
        session_bindings: plan.by_kind[1],
        turn_state_bindings: plan.by_kind[2],
        session_id_bindings: plan.by_kind[3],
        total_bindings: plan.total,
        profile_count: profiles.len(),
        profiles,
        missing_profile_bindings: plan.missing_profile_bindings,
        missing_profile_binding_samples,
        oldest_bound_at: plan.oldest_bound_at,
        newest_bound_at: plan.newest_bound_at,
    }
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
