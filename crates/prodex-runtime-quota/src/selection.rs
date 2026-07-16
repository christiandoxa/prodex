use crate::{
    RuntimeProfileUsageSnapshot, runtime_usage_snapshot_is_usable,
    usage_from_runtime_usage_snapshot,
};
use chrono::Local;
use prodex_quota::collect_blocked_limits;
use prodex_shared_types::{
    ReadyProfileCandidate, RunProfileProbeReport, RuntimeProfileProbeCacheEntry, RuntimeQuotaSource,
};
use prodex_state::{ProfileEntry, ProfileProvider};
use std::collections::BTreeMap;
use std::path::PathBuf;

mod scoring;

pub use self::scoring::*;

pub const RUN_SELECTION_NEAR_OPTIMAL_BPS: i64 = 1_000;
pub const RUN_SELECTION_HYSTERESIS_BPS: i64 = 500;
pub const RUN_SELECTION_COOLDOWN_SECONDS: i64 = 15 * 60;

pub trait ProfileSelectionProvider {
    fn runtime_pool_priority(&self) -> usize;
}

impl ProfileSelectionProvider for ProfileEntry {
    fn runtime_pool_priority(&self) -> usize {
        self.provider.runtime_pool_priority()
    }
}

pub trait ProfileSelectionRead: Copy {
    type Profile: ProfileSelectionProvider;

    fn profile_names(&self) -> Vec<String>;
    fn profile_entry(&self, name: &str) -> Option<&Self::Profile>;
    fn last_run_selected_at(&self, name: &str) -> Option<i64>;
}

pub struct ProfileSelectionView<'a, P> {
    pub profiles: &'a BTreeMap<String, P>,
    pub last_run_selected_at: &'a BTreeMap<String, i64>,
}

impl<P> Copy for ProfileSelectionView<'_, P> {}

impl<P> Clone for ProfileSelectionView<'_, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: ProfileSelectionProvider> ProfileSelectionRead for ProfileSelectionView<'_, P> {
    type Profile = P;

    fn profile_names(&self) -> Vec<String> {
        self.profiles.keys().cloned().collect()
    }

    fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
        self.profiles.get(name)
    }

    fn last_run_selected_at(&self, name: &str) -> Option<i64> {
        self.last_run_selected_at.get(name).copied()
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeSelectionProfileEntry {
    pub name: String,
    pub codex_home: PathBuf,
    pub provider: ProfileProvider,
    pub last_run_selected_at: Option<i64>,
}

impl ProfileSelectionProvider for RuntimeSelectionProfileEntry {
    fn runtime_pool_priority(&self) -> usize {
        self.provider.runtime_pool_priority()
    }
}

impl RuntimeSelectionProfileEntry {
    pub fn supports_codex_runtime(&self) -> bool {
        self.provider.supports_codex_runtime()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeProfileSelectionCatalogView<'a> {
    pub entries: &'a [RuntimeSelectionProfileEntry],
}

impl ProfileSelectionRead for RuntimeProfileSelectionCatalogView<'_> {
    type Profile = RuntimeSelectionProfileEntry;

    fn profile_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.name.clone())
            .collect()
    }

    fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    fn last_run_selected_at(&self, name: &str) -> Option<i64> {
        self.profile_entry(name)
            .and_then(|entry| entry.last_run_selected_at)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeProfileSelectionCatalog {
    pub entries: Vec<RuntimeSelectionProfileEntry>,
}

impl RuntimeProfileSelectionCatalog {
    pub fn contains(&self, profile_name: &str) -> bool {
        self.entries.iter().any(|entry| entry.name == profile_name)
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeRouteSelectionEntry {
    pub profile: RuntimeSelectionProfileEntry,
    pub cached_auth_summary: Option<prodex_quota::AuthSummary>,
    pub cached_probe_entry: Option<RuntimeProfileProbeCacheEntry>,
    pub cached_usage_snapshot: Option<RuntimeProfileUsageSnapshot>,
    pub auth_failure_active: bool,
    pub in_selection_backoff: bool,
    pub backoff_sort_key: (usize, i64, i64, i64),
    pub inflight_count: usize,
    pub health_sort_key: u32,
}

impl RuntimeRouteSelectionEntry {
    pub fn supports_codex_runtime(&self) -> bool {
        self.profile.supports_codex_runtime()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeRouteSelectionCatalogView<'a> {
    pub entries: &'a [RuntimeRouteSelectionEntry],
}

impl ProfileSelectionRead for RuntimeRouteSelectionCatalogView<'_> {
    type Profile = RuntimeSelectionProfileEntry;

    fn profile_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| entry.profile.name.clone())
            .collect()
    }

    fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
        self.entries
            .iter()
            .find(|entry| entry.profile.name == name)
            .map(|entry| &entry.profile)
    }

    fn last_run_selected_at(&self, name: &str) -> Option<i64> {
        self.profile_entry(name)
            .and_then(|entry| entry.last_run_selected_at)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRouteSelectionCatalog {
    pub current_profile: String,
    pub include_code_review: bool,
    pub upstream_base_url: String,
    pub entries: Vec<RuntimeRouteSelectionEntry>,
}

impl RuntimeRouteSelectionCatalog {
    pub fn entry(&self, profile_name: &str) -> Option<&RuntimeRouteSelectionEntry> {
        self.entries
            .iter()
            .find(|entry| entry.profile.name == profile_name)
    }

    pub fn persisted_usage_snapshots(&self) -> BTreeMap<String, RuntimeProfileUsageSnapshot> {
        self.entries
            .iter()
            .filter_map(|entry| {
                entry
                    .cached_usage_snapshot
                    .clone()
                    .map(|snapshot| (entry.profile.name.clone(), snapshot))
            })
            .collect()
    }
}

pub fn ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    preferred_profile: Option<&str>,
    selection: S,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
    stale_grace_seconds: i64,
) -> Vec<ReadyProfileCandidate> {
    let candidates = reports
        .iter()
        .filter_map(|report| {
            if !report.auth.quota_compatible {
                return None;
            }

            let (usage, quota_source) = match report.result.as_ref() {
                Ok(usage) => (usage.clone(), RuntimeQuotaSource::LiveProbe),
                Err(_) => {
                    let snapshot = persisted_usage_snapshots
                        .and_then(|snapshots| snapshots.get(&report.name))?;
                    let now = Local::now().timestamp();
                    if !runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds) {
                        return None;
                    }
                    (
                        usage_from_runtime_usage_snapshot(snapshot),
                        RuntimeQuotaSource::PersistedSnapshot,
                    )
                }
            };
            if !collect_blocked_limits(&usage, include_code_review).is_empty() {
                return None;
            }

            Some(ReadyProfileCandidate {
                name: report.name.clone(),
                usage,
                order_index: report.order_index,
                preferred: preferred_profile == Some(report.name.as_str()),
                provider_priority: selection
                    .profile_entry(&report.name)
                    .map(ProfileSelectionProvider::runtime_pool_priority)
                    .unwrap_or(usize::MAX),
                quota_source,
            })
        })
        .collect::<Vec<_>>();

    schedule_ready_profile_candidates_with_view(candidates, selection, preferred_profile)
}

pub fn run_profile_probe_is_ready(
    report: &RunProfileProbeReport,
    include_code_review: bool,
) -> bool {
    match report.result.as_ref() {
        Ok(usage) => collect_blocked_limits(usage, include_code_review).is_empty(),
        Err(_) => false,
    }
}

pub fn merge_run_preflight_reports_with_current_first(
    current_report: RunProfileProbeReport,
    rotation_reports: impl IntoIterator<Item = RunProfileProbeReport>,
) -> Vec<RunProfileProbeReport> {
    let mut reports = vec![current_report];
    reports.extend(rotation_reports.into_iter().map(|mut report| {
        report.order_index += 1;
        report
    }));
    reports
}
