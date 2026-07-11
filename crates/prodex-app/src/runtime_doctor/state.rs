use chrono::Local;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use prodex_runtime_doctor::{
    RuntimeDoctorBinaryIdentity, RuntimeDoctorBindingSourceInput, RuntimeDoctorBindingStateInput,
    RuntimeDoctorStateSummaryConfig, runtime_doctor_backoff_maps_from_runtime,
    runtime_doctor_health_scores_from_runtime, runtime_doctor_usage_snapshots_from_runtime,
};

fn runtime_doctor_state_summary_config() -> RuntimeDoctorStateSummaryConfig {
    RuntimeDoctorStateSummaryConfig {
        health_decay_seconds: RUNTIME_PROFILE_HEALTH_DECAY_SECONDS,
        bad_pairing_decay_seconds: RUNTIME_PROFILE_BAD_PAIRING_DECAY_SECONDS,
        performance_decay_seconds: RUNTIME_PROFILE_PERFORMANCE_DECAY_SECONDS,
        usage_snapshot_stale_grace_seconds: RUNTIME_PROFILE_USAGE_CACHE_STALE_GRACE_SECONDS,
    }
}

fn runtime_doctor_binary_identity(
    identity: &RuntimeProdexBinaryIdentity,
) -> RuntimeDoctorBinaryIdentity {
    RuntimeDoctorBinaryIdentity {
        prodex_version: identity.prodex_version.clone(),
        executable_path: identity
            .executable_path
            .as_ref()
            .map(|path| path.display().to_string()),
        executable_sha256: identity.executable_sha256.clone(),
    }
}

pub(crate) fn runtime_doctor_degraded_routes(
    backoffs: &RuntimeProfileBackoffs,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
    now: i64,
) -> Vec<String> {
    let scores = runtime_doctor_health_scores_from_runtime(scores);
    prodex_runtime_doctor::runtime_doctor_degraded_routes(
        runtime_doctor_backoff_maps_from_runtime(backoffs),
        &scores,
        now,
        runtime_doctor_state_summary_config(),
    )
}

struct RuntimeDoctorCollector {
    paths: Option<AppPaths>,
    pointer_path: PathBuf,
    pointed_log_path: Option<PathBuf>,
    newest_log_path: Option<PathBuf>,
    tail_bytes: usize,
}

impl RuntimeDoctorCollector {
    fn discover(tail_bytes: usize) -> Self {
        let pointer_path = runtime_proxy_latest_log_pointer_path();
        let pointed_log_path = runtime_proxy_latest_log_path_from_pointer();
        Self {
            paths: AppPaths::discover().ok(),
            pointer_path,
            pointed_log_path,
            newest_log_path: newest_runtime_proxy_log_in_dir(&runtime_proxy_log_dir()),
            tail_bytes,
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
        let mut summary = runtime_doctor_summary_from_log(log_path.as_deref(), self.tail_bytes);
        summary.pointer_exists = self.pointer_exists();
        summary.log_exists = log_path.as_ref().is_some_and(|path| path.exists());
        summary.log_path = log_path;
        if let Some(paths) = self.paths.as_ref() {
            collect_runtime_doctor_state(paths, &mut summary);
        }
        prodex_runtime_doctor::diagnosis::runtime_doctor_finalize_summary(&mut summary);
        prodex_runtime_doctor::diagnosis::runtime_doctor_append_pointer_note(
            &mut summary,
            self.pointer_note(),
        );
        summary
    }
}

fn runtime_doctor_profile_summaries(
    state: &AppState,
    usage_snapshots: &BTreeMap<String, RuntimeProfileUsageSnapshot>,
    scores: &BTreeMap<String, RuntimeProfileHealth>,
    backoffs: &RuntimeProfileBackoffs,
    now: i64,
) -> Vec<RuntimeDoctorProfileSummary> {
    let profile_names = state.profiles.keys().cloned().collect::<Vec<_>>();
    let usage_snapshots = runtime_doctor_usage_snapshots_from_runtime(usage_snapshots);
    let scores = runtime_doctor_health_scores_from_runtime(scores);
    prodex_runtime_doctor::runtime_doctor_profile_summaries(
        &profile_names,
        &usage_snapshots,
        &scores,
        runtime_doctor_backoff_maps_from_runtime(backoffs),
        now,
        runtime_doctor_state_summary_config(),
    )
}

fn runtime_doctor_path_prodex_binaries() -> Vec<PathBuf> {
    let executable_name = format!("prodex{}", env::consts::EXE_SUFFIX);
    let mut seen = BTreeSet::new();
    let mut binaries = Vec::new();
    if let Some(current_path) = runtime_current_prodex_binary_identity().executable_path {
        let normalized = fs::canonicalize(&current_path).unwrap_or(current_path);
        if seen.insert(normalized.clone()) {
            binaries.push(normalized);
        }
    }
    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let candidate = dir.join(&executable_name);
            if !candidate.is_file() {
                continue;
            }
            let normalized = fs::canonicalize(&candidate).unwrap_or(candidate);
            if seen.insert(normalized.clone()) {
                binaries.push(normalized);
            }
        }
    }
    binaries
}

fn runtime_doctor_binary_identity_line(path: &Path) -> Option<String> {
    let version = read_prodex_version_from_executable(path).ok()?;
    let sha256 = runtime_executable_sha256(path)
        .ok()
        .unwrap_or_else(|| "-".to_string());
    Some(format!(
        "{} version={} sha256={}",
        path.display(),
        version,
        sha256
    ))
}

fn runtime_doctor_collect_binary_identities(summary: &mut RuntimeDoctorSummary) {
    let current_version = runtime_current_prodex_version();
    let current_identity = runtime_current_prodex_binary_identity();
    let mut versions = BTreeSet::new();
    let mut hashes = BTreeSet::new();
    for path in runtime_doctor_path_prodex_binaries() {
        let Some(line) = runtime_doctor_binary_identity_line(&path) else {
            continue;
        };
        if let Some(version) = line
            .split_whitespace()
            .find_map(|token| token.strip_prefix("version="))
        {
            versions.insert(version.to_string());
        }
        if let Some(hash) = line
            .split_whitespace()
            .find_map(|token| token.strip_prefix("sha256="))
            .filter(|value| *value != "-")
        {
            hashes.insert(hash.to_string());
        }
        summary.prodex_binary_identities.push(line);
    }
    summary.prodex_binary_mismatch = versions.iter().any(|version| version != current_version)
        || hashes.len() > 1
        || current_identity
            .executable_sha256
            .as_ref()
            .is_some_and(|hash| !hashes.is_empty() && !hashes.contains(hash));
}

fn runtime_doctor_count_stale_runtime_broker_leases(paths: &AppPaths, broker_key: &str) -> usize {
    let lease_dir = runtime_broker_lease_dir(paths, broker_key);
    if !runtime_broker_lease_dir_is_regular_dir(&lease_dir) {
        return 0;
    }
    let Ok(entries) = fs::read_dir(&lease_dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            if !runtime_broker_lease_path_is_regular_file(&path) {
                return false;
            }
            let pid = file_name
                .split('-')
                .next()
                .and_then(|value| value.parse::<u32>().ok());
            !pid.is_some_and(runtime_process_pid_alive)
        })
        .count()
}

fn runtime_doctor_probe_runtime_broker_health_status(
    client: &Client,
    registry: &RuntimeBrokerRegistry,
) -> (&'static str, Option<RuntimeBrokerHealth>) {
    let response = client
        .get(runtime_broker_health_url(registry))
        .header("X-Prodex-Admin-Token", &registry.admin_token)
        .send();
    match response {
        Ok(response) => {
            if !response.status().is_success() {
                return ("health_unreachable", None);
            }
            let body = read_blocking_response_body_with_limit(
                response,
                RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
                "failed to read runtime broker health response",
            );
            match body
                .as_deref()
                .ok()
                .and_then(|body| serde_json::from_slice::<RuntimeBrokerHealth>(body).ok())
            {
                Some(health) => ("healthy", Some(health)),
                None => ("health_unreachable", None),
            }
        }
        Err(err) if err.is_timeout() => ("health_timeout", None),
        Err(_) => ("health_unreachable", None),
    }
}

fn runtime_doctor_collect_broker_identities(paths: &AppPaths, summary: &mut RuntimeDoctorSummary) {
    let current_identity = runtime_current_prodex_binary_identity();
    let current_doctor_identity = runtime_doctor_binary_identity(&current_identity);
    let client = runtime_broker_client().ok();
    for broker_key in runtime_broker_registry_keys(paths) {
        let Ok(Some(registry)) = load_runtime_broker_registry(paths, &broker_key) else {
            continue;
        };
        let stale_leases = runtime_doctor_count_stale_runtime_broker_leases(paths, &broker_key);
        if !runtime_process_pid_alive(registry.pid) {
            summary.runtime_broker_identities.push(format!(
                "broker_key={} pid={} listen_addr={} status=dead_pid mismatch=none version={} path={} sha256={} source=registry stale_leases={}",
                broker_key,
                registry.pid,
                registry.listen_addr,
                registry.prodex_version.clone().unwrap_or_else(|| "-".to_string()),
                registry.executable_path.clone().unwrap_or_else(|| "-".to_string()),
                registry.executable_sha256.clone().unwrap_or_else(|| "-".to_string()),
                stale_leases
            ));
            continue;
        }
        let (health_status, health) = client
            .as_ref()
            .map(|client| runtime_doctor_probe_runtime_broker_health_status(client, &registry))
            .unwrap_or(("health_unreachable", None));
        let (identity, source) = health
            .as_ref()
            .map(|health| (runtime_health_prodex_binary_identity(health), "health"))
            .filter(|(identity, _)| identity.is_present())
            .or_else(|| {
                let identity = runtime_registry_prodex_binary_identity(&registry);
                identity.is_present().then_some((identity, "registry"))
            })
            .or_else(|| {
                let identity = runtime_process_prodex_binary_identity(registry.pid);
                identity.is_present().then_some((identity, "process"))
            })
            .unwrap_or((RuntimeProdexBinaryIdentity::default(), "missing"));
        let mismatch = if identity.is_present()
            && !runtime_prodex_binary_identity_matches(&current_identity, &identity)
        {
            summary.runtime_broker_mismatch = true;
            prodex_runtime_doctor::runtime_doctor_runtime_broker_mismatch_reason(
                &current_doctor_identity,
                &runtime_doctor_binary_identity(&identity),
            )
        } else {
            "none"
        };
        let status = if mismatch != "none" {
            "binary_mismatch"
        } else {
            health_status
        };
        summary.runtime_broker_identities.push(format!(
            "broker_key={} pid={} listen_addr={} status={} mismatch={} version={} path={} sha256={} source={} stale_leases={}",
            broker_key,
            registry.pid,
            registry.listen_addr,
            status,
            mismatch,
            identity.prodex_version.unwrap_or_else(|| "-".to_string()),
            identity
                .executable_path
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_string()),
            identity
                .executable_sha256
                .unwrap_or_else(|| "-".to_string()),
            source,
            stale_leases
        ));
    }
}

pub(crate) fn collect_runtime_doctor_state(paths: &AppPaths, summary: &mut RuntimeDoctorSummary) {
    let Ok(state) = AppState::load_with_recovery(paths) else {
        runtime_doctor_collect_binary_identities(summary);
        runtime_doctor_collect_broker_identities(paths, summary);
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
    let profile_names = state.value.profiles.keys().cloned().collect::<Vec<_>>();
    let empty_bindings = BTreeMap::new();
    summary.binding_state = prodex_runtime_doctor::runtime_doctor_binding_state_summary(
        RuntimeDoctorBindingStateInput {
            active_profile: state.value.active_profile.as_deref(),
            profile_names: &profile_names,
            last_run_selected_profiles: state.value.last_run_selected_at.len(),
            state: RuntimeDoctorBindingSourceInput {
                response_profile_bindings: &state.value.response_profile_bindings,
                session_profile_bindings: &state.value.session_profile_bindings,
                turn_state_bindings: &empty_bindings,
                session_id_bindings: &empty_bindings,
            },
            runtime_continuations: RuntimeDoctorBindingSourceInput {
                response_profile_bindings: &continuations.value.response_profile_bindings,
                session_profile_bindings: &continuations.value.session_profile_bindings,
                turn_state_bindings: &continuations.value.turn_state_bindings,
                session_id_bindings: &continuations.value.session_id_bindings,
            },
            continuation_journal: RuntimeDoctorBindingSourceInput {
                response_profile_bindings: &continuation_journal
                    .value
                    .continuations
                    .response_profile_bindings,
                session_profile_bindings: &continuation_journal
                    .value
                    .continuations
                    .session_profile_bindings,
                turn_state_bindings: &continuation_journal.value.continuations.turn_state_bindings,
                session_id_bindings: &continuation_journal.value.continuations.session_id_bindings,
            },
            merged_continuations: RuntimeDoctorBindingSourceInput {
                response_profile_bindings: &merged_continuations.response_profile_bindings,
                session_profile_bindings: &merged_continuations.session_profile_bindings,
                turn_state_bindings: &merged_continuations.turn_state_bindings,
                session_id_bindings: &merged_continuations.session_id_bindings,
            },
        },
        |binding| binding.profile_name.as_str(),
        |binding| binding.bound_at,
    );

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
    let merged_response_bindings =
        runtime_external_response_profile_bindings(&merged_continuations.response_profile_bindings)
            .len();
    summary.persisted_turn_state_coverage_percent =
        std::num::NonZeroUsize::new(merged_response_bindings).map(|divisor| {
            ((merged_continuations.turn_state_bindings.len() * 100) / divisor.get()).min(100) as u8
        });
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
    runtime_doctor_collect_binary_identities(summary);
    runtime_doctor_collect_broker_identities(paths, summary);
}

pub(crate) fn collect_runtime_doctor_summary_with_tail_bytes(
    tail_bytes: usize,
) -> RuntimeDoctorSummary {
    RuntimeDoctorCollector::discover(tail_bytes).collect()
}

fn runtime_doctor_summary_from_log(
    log_path: Option<&Path>,
    tail_bytes: usize,
) -> RuntimeDoctorSummary {
    if let Some(log_path) = log_path.filter(|path| path.exists()) {
        match prodex_runtime_doctor::read_runtime_log_tail(log_path, tail_bytes) {
            Ok(tail) => prodex_runtime_doctor::summarize_runtime_log_tail(&tail),
            Err(err) => RuntimeDoctorSummary {
                diagnosis: format!("Failed to read the latest runtime log tail: {err}"),
                ..RuntimeDoctorSummary::default()
            },
        }
    } else {
        RuntimeDoctorSummary::default()
    }
}
