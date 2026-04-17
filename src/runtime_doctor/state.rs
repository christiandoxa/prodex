use chrono::Local;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

fn runtime_doctor_push_route_circuits(
    routes: &mut Vec<String>,
    backoffs: &RuntimeProfileBackoffs,
    now: i64,
) {
    for (key, until) in &backoffs.route_circuit_open_until {
        if let Some((route, profile_name)) =
            runtime_profile_route_key_parts(key, "__route_circuit__:")
        {
            let state = if *until > now { "open" } else { "half-open" };
            routes.push(format!(
                "{profile_name}/{route} circuit={state} until={until}"
            ));
        }
    }
}

fn runtime_doctor_push_transport_backoffs(
    routes: &mut Vec<String>,
    backoffs: &RuntimeProfileBackoffs,
) {
    for (profile_name, until) in &backoffs.transport_backoff_until {
        if let Some((route, profile_name)) =
            runtime_profile_transport_backoff_key_parts(profile_name)
        {
            routes.push(format!(
                "{profile_name}/{route} transport_backoff until={until}"
            ));
        } else {
            routes.push(format!(
                "{profile_name}/transport transport_backoff until={until}"
            ));
        }
    }
}

fn runtime_doctor_push_retry_backoffs(routes: &mut Vec<String>, backoffs: &RuntimeProfileBackoffs) {
    for (profile_name, until) in &backoffs.retry_backoff_until {
        routes.push(format!("{profile_name}/retry retry_backoff until={until}"));
    }
}

fn runtime_doctor_push_score_degradations(
    routes: &mut Vec<String>,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) {
    for (key, health) in scores {
        if let Some((route, profile_name)) =
            runtime_profile_route_key_parts(key, "__route_bad_pairing__:")
        {
            let score = runtime_profile_effective_score(
                health,
                now,
                RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
            );
            if score > 0 {
                routes.push(format!("{profile_name}/{route} bad_pairing={score}"));
            }
            continue;
        }
        if let Some((route, profile_name)) =
            runtime_profile_route_key_parts(key, "__route_health__:")
        {
            let score = runtime_profile_effective_health_score(health, now);
            if score > 0 {
                routes.push(format!("{profile_name}/{route} health={score}"));
            }
        }
    }
}

pub(crate) fn runtime_doctor_degraded_routes(
    backoffs: &RuntimeProfileBackoffs,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> Vec<String> {
    let mut routes = Vec::new();
    runtime_doctor_push_route_circuits(&mut routes, backoffs, now);
    runtime_doctor_push_transport_backoffs(&mut routes, backoffs);
    runtime_doctor_push_retry_backoffs(&mut routes, backoffs);
    runtime_doctor_push_score_degradations(&mut routes, scores, now);
    routes.sort();
    routes.dedup();
    routes.truncate(8);
    routes
}

struct RuntimeDoctorCollector {
    paths: Option<AppPaths>,
    pointer_path: PathBuf,
    pointed_log_path: Option<PathBuf>,
    newest_log_path: Option<PathBuf>,
}

impl RuntimeDoctorCollector {
    fn discover() -> Self {
        let pointer_path = runtime_proxy_latest_log_pointer_path();
        let pointed_log_path = fs::read_to_string(&pointer_path)
            .ok()
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self {
            paths: AppPaths::discover().ok(),
            pointer_path,
            pointed_log_path,
            newest_log_path: newest_runtime_proxy_log_in_dir(&runtime_proxy_log_dir()),
        }
    }

    fn pointer_exists(&self) -> bool {
        self.pointer_path.exists()
    }

    fn pointer_target_exists(&self) -> bool {
        self.pointed_log_path
            .as_ref()
            .is_some_and(|path| path.exists())
    }

    fn pointer_note(&self) -> Option<&'static str> {
        match (
            self.pointed_log_path.as_ref(),
            self.newest_log_path.as_ref(),
        ) {
            (Some(pointed), Some(newest)) if pointed.exists() && newest != pointed => {
                Some("Runtime log pointer was stale; sampled a newer log instead.")
            }
            (Some(_), Some(_)) if !self.pointer_target_exists() => {
                Some("Runtime log pointer target was missing; sampled a newer log instead.")
            }
            (Some(_), None) if !self.pointer_target_exists() => {
                Some("Runtime log pointer target was missing.")
            }
            _ => None,
        }
    }

    fn log_path(&self) -> Option<PathBuf> {
        if self.pointer_target_exists() {
            self.newest_log_path
                .as_ref()
                .filter(|path| {
                    self.pointed_log_path
                        .as_ref()
                        .is_some_and(|pointed| *path != pointed)
                })
                .cloned()
                .or_else(|| self.pointed_log_path.clone())
        } else {
            self.newest_log_path.clone()
        }
    }

    fn collect(self) -> RuntimeDoctorSummary {
        let log_path = self.log_path();
        let mut summary = runtime_doctor_summary_from_log(log_path.as_deref());
        summary.pointer_exists = self.pointer_exists();
        summary.log_exists = log_path.as_ref().is_some_and(|path| path.exists());
        summary.log_path = log_path;
        if let Some(paths) = self.paths.as_ref() {
            collect_runtime_doctor_state(paths, &mut summary);
        }
        diagnosis::runtime_doctor_finalize_summary(&mut summary);
        diagnosis::runtime_doctor_append_pointer_note(&mut summary, self.pointer_note());
        summary
    }
}

fn runtime_doctor_quota_freshness_label(
    snapshot: Option<&RuntimeProfileUsageSnapshot>,
    now: i64,
) -> String {
    match snapshot {
        Some(snapshot) if runtime_usage_snapshot_is_usable(snapshot, now) => "fresh".to_string(),
        Some(_) => "stale".to_string(),
        None => "missing".to_string(),
    }
}

fn runtime_doctor_route_circuit_state(until: Option<i64>, now: i64) -> String {
    match until {
        Some(until) if until > now => "open".to_string(),
        Some(_) => "half_open".to_string(),
        None => "closed".to_string(),
    }
}

fn runtime_doctor_profile_summaries(
    state: &AppState,
    usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
    backoffs: &RuntimeProfileBackoffs,
    now: i64,
) -> Vec<RuntimeDoctorProfileSummary> {
    let mut profiles = Vec::new();
    for profile_name in state.profiles.keys() {
        let snapshot = usage_snapshots.get(profile_name);
        let quota_age_seconds = snapshot
            .map(|snapshot| now.saturating_sub(snapshot.checked_at))
            .unwrap_or(i64::MAX);
        let routes = [
            RuntimeRouteKind::Responses,
            RuntimeRouteKind::Websocket,
            RuntimeRouteKind::Compact,
            RuntimeRouteKind::Standard,
        ]
        .into_iter()
        .map(|route_kind| {
            let quota_summary = snapshot
                .map(|snapshot| runtime_quota_summary_from_usage_snapshot(snapshot, route_kind))
                .unwrap_or(RuntimeQuotaSummary {
                    five_hour: RuntimeQuotaWindowSummary {
                        status: RuntimeQuotaWindowStatus::Unknown,
                        remaining_percent: 0,
                        reset_at: i64::MAX,
                    },
                    weekly: RuntimeQuotaWindowSummary {
                        status: RuntimeQuotaWindowStatus::Unknown,
                        remaining_percent: 0,
                        reset_at: i64::MAX,
                    },
                    route_band: RuntimeQuotaPressureBand::Unknown,
                });
            RuntimeDoctorRouteSummary {
                route: runtime_route_kind_label(route_kind).to_string(),
                circuit_state: runtime_doctor_route_circuit_state(
                    backoffs
                        .route_circuit_open_until
                        .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
                        .copied(),
                    now,
                ),
                circuit_until: backoffs
                    .route_circuit_open_until
                    .get(&runtime_profile_route_circuit_key(profile_name, route_kind))
                    .copied(),
                transport_backoff_until: runtime_profile_transport_backoff_until_from_map(
                    &backoffs.transport_backoff_until,
                    profile_name,
                    route_kind,
                    now,
                ),
                health_score: runtime_profile_effective_health_score_from_map(
                    scores,
                    &runtime_profile_route_health_key(profile_name, route_kind),
                    now,
                ),
                bad_pairing_score: runtime_profile_effective_score_from_map(
                    scores,
                    &runtime_profile_route_bad_pairing_key(profile_name, route_kind),
                    now,
                    RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
                ),
                performance_score: runtime_profile_effective_score_from_map(
                    scores,
                    &runtime_profile_route_performance_key(profile_name, route_kind),
                    now,
                    RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
                ),
                quota_band: runtime_quota_pressure_band_reason(quota_summary.route_band)
                    .to_string(),
                five_hour_status: runtime_quota_window_status_reason(
                    quota_summary.five_hour.status,
                )
                .to_string(),
                weekly_status: runtime_quota_window_status_reason(quota_summary.weekly.status)
                    .to_string(),
            }
        })
        .collect::<Vec<_>>();
        profiles.push(RuntimeDoctorProfileSummary {
            profile: profile_name.clone(),
            quota_freshness: runtime_doctor_quota_freshness_label(snapshot, now),
            quota_age_seconds,
            retry_backoff_until: backoffs.retry_backoff_until.get(profile_name).copied(),
            transport_backoff_until: runtime_profile_transport_backoff_max_until(
                &backoffs.transport_backoff_until,
                profile_name,
                now,
            ),
            routes,
        });
    }
    profiles
}

pub(crate) fn collect_runtime_doctor_state(paths: &AppPaths, summary: &mut RuntimeDoctorSummary) {
    let Ok(state) = AppState::load_with_recovery(paths) else {
        return;
    };
    let now = Local::now().timestamp();
    let usage_snapshots = load_runtime_usage_snapshots_with_recovery(paths, &state.value.profiles)
        .unwrap_or(RecoveredLoad {
            value: BTreeMap::new(),
            recovered_from_backup: false,
        });
    let scores = load_runtime_profile_scores_with_recovery(paths, &state.value.profiles).unwrap_or(
        RecoveredLoad {
            value: BTreeMap::new(),
            recovered_from_backup: false,
        },
    );
    let continuations = load_runtime_continuations_with_recovery(paths, &state.value.profiles)
        .unwrap_or(RecoveredLoad {
            value: RuntimeContinuationStore::default(),
            recovered_from_backup: false,
        });
    let continuation_journal =
        load_runtime_continuation_journal_with_recovery(paths, &state.value.profiles).unwrap_or(
            RecoveredLoad {
                value: RuntimeContinuationJournal::default(),
                recovered_from_backup: false,
            },
        );
    let merged_continuations = merge_runtime_continuation_store(
        &continuations.value,
        &continuation_journal.value.continuations,
        &state.value.profiles,
    );
    let backoffs = load_runtime_profile_backoffs_with_recovery(paths, &state.value.profiles)
        .unwrap_or(RecoveredLoad {
            value: RuntimeProfileBackoffs::default(),
            recovered_from_backup: false,
        });
    let orphan_managed_dirs = collect_orphan_managed_profile_dirs(paths, &state.value);

    summary.persisted_retry_backoffs = backoffs.value.retry_backoff_until.len();
    summary.persisted_transport_backoffs = backoffs.value.transport_backoff_until.len();
    summary.persisted_route_circuits = backoffs.value.route_circuit_open_until.len();
    summary.persisted_usage_snapshots = usage_snapshots.value.len();
    summary.persisted_response_bindings =
        runtime_external_response_profile_bindings(&continuations.value.response_profile_bindings)
            .len();
    summary.persisted_session_bindings = continuations.value.session_profile_bindings.len();
    summary.persisted_turn_state_bindings = continuations.value.turn_state_bindings.len();
    summary.persisted_session_id_bindings = continuations.value.session_id_bindings.len();
    summary.persisted_continuation_journal_response_bindings = continuation_journal
        .value
        .continuations
        .response_profile_bindings
        .len();
    summary.persisted_continuation_journal_session_bindings = continuation_journal
        .value
        .continuations
        .session_profile_bindings
        .len();
    summary.persisted_continuation_journal_turn_state_bindings = continuation_journal
        .value
        .continuations
        .turn_state_bindings
        .len();
    summary.persisted_continuation_journal_session_id_bindings = continuation_journal
        .value
        .continuations
        .session_id_bindings
        .len();
    summary.continuation_journal_saved_at =
        (continuation_journal.value.saved_at > 0).then_some(continuation_journal.value.saved_at);
    summary.stale_persisted_usage_snapshots = usage_snapshots
        .value
        .values()
        .filter(|snapshot| !runtime_usage_snapshot_is_usable(snapshot, now))
        .count();
    summary.recovered_state_file = state.recovered_from_backup;
    summary.recovered_continuations_file = continuations.recovered_from_backup;
    summary.recovered_scores_file = scores.recovered_from_backup;
    summary.recovered_usage_snapshots_file = usage_snapshots.recovered_from_backup;
    summary.recovered_backoffs_file = backoffs.recovered_from_backup;
    summary.recovered_continuation_journal_file = continuation_journal.recovered_from_backup;
    summary.last_good_backups_present = [
        state_last_good_file_path(paths),
        runtime_continuations_last_good_file_path(paths),
        runtime_continuation_journal_last_good_file_path(paths),
        runtime_scores_last_good_file_path(paths),
        runtime_usage_snapshots_last_good_file_path(paths),
        runtime_backoffs_last_good_file_path(paths),
    ]
    .into_iter()
    .filter(|path| path.exists())
    .count();
    for statuses in [
        &merged_continuations.statuses.response,
        &merged_continuations.statuses.turn_state,
        &merged_continuations.statuses.session_id,
    ] {
        for (key, status) in statuses {
            match status.state {
                RuntimeContinuationBindingLifecycle::Verified => {
                    summary.persisted_verified_continuations += 1;
                }
                RuntimeContinuationBindingLifecycle::Warm => {
                    summary.persisted_warm_continuations += 1;
                }
                RuntimeContinuationBindingLifecycle::Suspect => {
                    summary.persisted_suspect_continuations += 1;
                    summary.suspect_continuation_bindings.push(format!(
                        "{key}:{}",
                        runtime_continuation_status_label(status)
                    ));
                }
                RuntimeContinuationBindingLifecycle::Dead => {
                    summary.persisted_dead_continuations += 1;
                }
            }
        }
    }
    summary.suspect_continuation_bindings.sort();
    summary.orphan_managed_dirs = orphan_managed_dirs;
    summary.profiles = runtime_doctor_profile_summaries(
        &state.value,
        &usage_snapshots.value,
        &scores.value,
        &backoffs.value,
        now,
    );
    summary.degraded_routes = runtime_doctor_degraded_routes(&backoffs.value, &scores.value, now);
}

pub(crate) fn collect_runtime_doctor_summary() -> RuntimeDoctorSummary {
    RuntimeDoctorCollector::discover().collect()
}

fn runtime_doctor_summary_from_log(log_path: Option<&Path>) -> RuntimeDoctorSummary {
    if let Some(log_path) = log_path.filter(|path| path.exists()) {
        match parsing::read_runtime_log_tail(log_path, RUNTIME_PROXY_DOCTOR_TAIL_BYTES) {
            Ok(tail) => parsing::summarize_runtime_log_tail(&tail),
            Err(err) => RuntimeDoctorSummary {
                diagnosis: format!("Failed to read the latest runtime log tail: {err}"),
                ..RuntimeDoctorSummary::default()
            },
        }
    } else {
        RuntimeDoctorSummary::default()
    }
}
