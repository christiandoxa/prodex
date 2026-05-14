use crate::info::{
    RuntimeProfileUsageSnapshot, required_main_window_snapshot_at,
    runtime_usage_snapshot_is_usable, usage_from_runtime_usage_snapshot,
};
use chrono::Local;
use prodex_quota::{RuntimeQuotaPressureBand, UsageResponse, collect_blocked_limits};
use prodex_runtime_state::RuntimeRouteKind;
use prodex_shared_types::{
    ReadyProfileCandidate, ReadyProfileScore, RunProfileProbeReport, RuntimeProfileProbeCacheEntry,
    RuntimeQuotaSource,
};
use prodex_state::{ProfileEntry, ProfileProvider};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::PathBuf;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSelectionDecision {
    Selected,
    Ready,
    Rejected,
    MissingReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSelectionReasonKind {
    ActiveCurrentPreference,
    PreferredProfile,
    QuotaReady,
    AuthIncompatible,
    ProviderPriority,
    RecentlySelected,
    MissingReport,
    Blocked,
    NotReady,
    MissingPersistedSnapshot,
    PersistedSnapshotUsed,
    LiveProbeUsed,
    SelectionBackoff,
    AuthFailureActive,
    SupportsCodexRuntime,
    UnsupportedCodexRuntime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSelectionReason {
    pub kind: ProfileSelectionReasonKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SelectionQuotaSourceView {
    LiveProbe,
    PersistedSnapshot,
}

impl From<RuntimeQuotaSource> for SelectionQuotaSourceView {
    fn from(source: RuntimeQuotaSource) -> Self {
        match source {
            RuntimeQuotaSource::LiveProbe => Self::LiveProbe,
            RuntimeQuotaSource::PersistedSnapshot => Self::PersistedSnapshot,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSelectionScoreView {
    pub total_pressure: i64,
    pub weekly_pressure: i64,
    pub five_hour_pressure: i64,
    pub reserve_floor: i64,
    pub weekly_remaining: i64,
    pub five_hour_remaining: i64,
    pub weekly_reset_at: i64,
    pub five_hour_reset_at: i64,
}

impl From<ReadyProfileScore> for ProfileSelectionScoreView {
    fn from(score: ReadyProfileScore) -> Self {
        Self {
            total_pressure: score.total_pressure,
            weekly_pressure: score.weekly_pressure,
            five_hour_pressure: score.five_hour_pressure,
            reserve_floor: score.reserve_floor,
            weekly_remaining: score.weekly_remaining,
            five_hour_remaining: score.five_hour_remaining,
            weekly_reset_at: score.weekly_reset_at,
            five_hour_reset_at: score.five_hour_reset_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSelectionCandidateExplanation {
    pub order_index: usize,
    pub preferred: bool,
    pub provider_priority: usize,
    pub quota_source: SelectionQuotaSourceView,
    pub score: ProfileSelectionScoreView,
    pub recently_selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSelectionProfileExplanation {
    pub name: String,
    pub decision: ProfileSelectionDecision,
    pub rank: Option<usize>,
    pub selected: bool,
    pub last_run_selected_at: Option<i64>,
    pub candidate: Option<ProfileSelectionCandidateExplanation>,
    pub blocked_limits: Vec<String>,
    pub probe_error: Option<String>,
    pub reasons: Vec<ProfileSelectionReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSelectionExplanation {
    pub selected_profile: Option<String>,
    pub current_profile: Option<String>,
    pub preferred_profile: Option<String>,
    pub profiles: Vec<ProfileSelectionProfileExplanation>,
}

pub fn explain_ready_profile_selection_with_view<S: ProfileSelectionRead>(
    reports: &[RunProfileProbeReport],
    include_code_review: bool,
    current_profile: Option<&str>,
    preferred_profile: Option<&str>,
    selection: S,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
    stale_grace_seconds: i64,
) -> ProfileSelectionExplanation {
    let candidates = ready_profile_candidates_with_view(
        reports,
        include_code_review,
        preferred_profile,
        selection,
        persisted_usage_snapshots,
        stale_grace_seconds,
    );
    explain_ready_profile_candidates_with_view(
        reports,
        &candidates,
        include_code_review,
        current_profile,
        preferred_profile,
        selection,
        persisted_usage_snapshots,
        stale_grace_seconds,
    )
}

pub fn explain_ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    reports: &[RunProfileProbeReport],
    scheduled_candidates: &[ReadyProfileCandidate],
    include_code_review: bool,
    current_profile: Option<&str>,
    preferred_profile: Option<&str>,
    selection: S,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
    stale_grace_seconds: i64,
) -> ProfileSelectionExplanation {
    let now = Local::now().timestamp();
    let selected_profile = scheduled_candidates
        .first()
        .map(|candidate| candidate.name.clone());
    let candidate_ranks = scheduled_candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| (candidate.name.as_str(), (index, candidate)))
        .collect::<BTreeMap<_, _>>();
    let report_by_name = reports
        .iter()
        .map(|report| (report.name.as_str(), report))
        .collect::<BTreeMap<_, _>>();
    let mut names = selection.profile_names();
    for report in reports {
        if !names.iter().any(|name| name == &report.name) {
            names.push(report.name.clone());
        }
    }

    let profiles = names
        .into_iter()
        .map(|name| {
            explain_profile_selection_entry(
                &name,
                report_by_name.get(name.as_str()).copied(),
                candidate_ranks.get(name.as_str()).copied(),
                include_code_review,
                current_profile,
                preferred_profile,
                selection,
                persisted_usage_snapshots,
                stale_grace_seconds,
                now,
                selected_profile.as_deref(),
            )
        })
        .collect();

    ProfileSelectionExplanation {
        selected_profile,
        current_profile: current_profile.map(str::to_string),
        preferred_profile: preferred_profile.map(str::to_string),
        profiles,
    }
}

pub fn explain_runtime_profile_selection_catalog(
    catalog: &RuntimeProfileSelectionCatalog,
    current_profile: Option<&str>,
    preferred_profile: Option<&str>,
) -> ProfileSelectionExplanation {
    explain_ready_profile_candidates_with_view(
        &[],
        &[],
        false,
        current_profile,
        preferred_profile,
        RuntimeProfileSelectionCatalogView {
            entries: &catalog.entries,
        },
        None,
        0,
    )
}

pub fn explain_runtime_route_selection_catalog(
    catalog: &RuntimeRouteSelectionCatalog,
    preferred_profile: Option<&str>,
    stale_grace_seconds: i64,
) -> ProfileSelectionExplanation {
    let reports = catalog
        .entries
        .iter()
        .map(|entry| RunProfileProbeReport {
            name: entry.profile.name.clone(),
            order_index: entry.profile.runtime_pool_priority(),
            auth: entry
                .cached_auth_summary
                .clone()
                .or_else(|| {
                    entry
                        .cached_probe_entry
                        .as_ref()
                        .map(|probe| probe.auth.clone())
                })
                .unwrap_or(prodex_quota::AuthSummary {
                    label: "missing".to_string(),
                    quota_compatible: false,
                }),
            result: entry
                .cached_probe_entry
                .as_ref()
                .map(|probe| probe.result.clone())
                .unwrap_or_else(|| Err("missing probe report".to_string())),
        })
        .collect::<Vec<_>>();
    let mut explanation = explain_ready_profile_selection_with_view(
        &reports,
        catalog.include_code_review,
        Some(&catalog.current_profile),
        preferred_profile,
        RuntimeRouteSelectionCatalogView {
            entries: &catalog.entries,
        },
        Some(&catalog.persisted_usage_snapshots()),
        stale_grace_seconds,
    );

    for profile in &mut explanation.profiles {
        if let Some(entry) = catalog.entry(&profile.name) {
            if entry.auth_failure_active {
                profile.decision = ProfileSelectionDecision::Rejected;
                profile.reasons.push(selection_reason(
                    ProfileSelectionReasonKind::AuthFailureActive,
                    "auth failure is active for this profile",
                ));
            }
            if entry.in_selection_backoff {
                profile.decision = ProfileSelectionDecision::Rejected;
                profile.reasons.push(selection_reason(
                    ProfileSelectionReasonKind::SelectionBackoff,
                    "profile is in selection backoff",
                ));
            }
            if entry.supports_codex_runtime() {
                profile.reasons.push(selection_reason(
                    ProfileSelectionReasonKind::SupportsCodexRuntime,
                    "profile provider supports Codex runtime",
                ));
            } else {
                profile.decision = ProfileSelectionDecision::Rejected;
                profile.reasons.push(selection_reason(
                    ProfileSelectionReasonKind::UnsupportedCodexRuntime,
                    "profile provider does not support Codex runtime",
                ));
            }
        }
    }
    explanation.selected_profile = explanation
        .profiles
        .iter()
        .find(|profile| profile.selected && profile.decision == ProfileSelectionDecision::Selected)
        .map(|profile| profile.name.clone());
    explanation
}

fn explain_profile_selection_entry<S: ProfileSelectionRead>(
    name: &str,
    report: Option<&RunProfileProbeReport>,
    ranked_candidate: Option<(usize, &ReadyProfileCandidate)>,
    include_code_review: bool,
    current_profile: Option<&str>,
    preferred_profile: Option<&str>,
    selection: S,
    persisted_usage_snapshots: Option<&BTreeMap<String, RuntimeProfileUsageSnapshot>>,
    stale_grace_seconds: i64,
    now: i64,
    selected_profile: Option<&str>,
) -> ProfileSelectionProfileExplanation {
    let selected = selected_profile == Some(name);
    let last_run_selected_at = selection.last_run_selected_at(name);
    let mut reasons = Vec::new();
    if current_profile == Some(name) {
        reasons.push(selection_reason(
            ProfileSelectionReasonKind::ActiveCurrentPreference,
            "profile is the active/current profile",
        ));
    }
    if preferred_profile == Some(name) {
        reasons.push(selection_reason(
            ProfileSelectionReasonKind::PreferredProfile,
            "profile was explicitly preferred",
        ));
    }
    if let Some(priority) = selection
        .profile_entry(name)
        .map(ProfileSelectionProvider::runtime_pool_priority)
    {
        reasons.push(selection_reason(
            ProfileSelectionReasonKind::ProviderPriority,
            format!("provider priority {priority}"),
        ));
    }
    if profile_in_run_selection_cooldown_with_view(selection, name, now) {
        reasons.push(selection_reason(
            ProfileSelectionReasonKind::RecentlySelected,
            "profile was selected recently",
        ));
    }

    let mut blocked_limits = Vec::new();
    let mut probe_error = None;
    let candidate = ranked_candidate.map(|(_, candidate)| {
        reasons.push(selection_reason(
            ProfileSelectionReasonKind::QuotaReady,
            "profile has compatible auth and unblocked quota",
        ));
        reasons.push(match candidate.quota_source {
            RuntimeQuotaSource::LiveProbe => selection_reason(
                ProfileSelectionReasonKind::LiveProbeUsed,
                "quota readiness came from a live probe",
            ),
            RuntimeQuotaSource::PersistedSnapshot => selection_reason(
                ProfileSelectionReasonKind::PersistedSnapshotUsed,
                "quota readiness came from a persisted snapshot",
            ),
        });
        ProfileSelectionCandidateExplanation {
            order_index: candidate.order_index,
            preferred: candidate.preferred,
            provider_priority: candidate.provider_priority,
            quota_source: candidate.quota_source.into(),
            score: ready_profile_score(candidate).into(),
            recently_selected: profile_in_run_selection_cooldown_with_view(selection, name, now),
        }
    });

    if candidate.is_none() {
        match report {
            None => reasons.push(selection_reason(
                ProfileSelectionReasonKind::MissingReport,
                "no selection report exists for this profile",
            )),
            Some(report) if !report.auth.quota_compatible => reasons.push(selection_reason(
                ProfileSelectionReasonKind::AuthIncompatible,
                format!("auth label '{}' is not quota compatible", report.auth.label),
            )),
            Some(report) => match report.result.as_ref() {
                Ok(usage) => {
                    blocked_limits = collect_blocked_limits(usage, include_code_review)
                        .into_iter()
                        .map(|limit| limit.message)
                        .collect();
                    reasons.push(if blocked_limits.is_empty() {
                        selection_reason(
                            ProfileSelectionReasonKind::NotReady,
                            "profile was not scheduled as a ready candidate",
                        )
                    } else {
                        selection_reason(
                            ProfileSelectionReasonKind::Blocked,
                            format!("blocked quota: {}", blocked_limits.join(", ")),
                        )
                    });
                }
                Err(error) => {
                    probe_error = Some(error.clone());
                    let snapshot_usable = persisted_usage_snapshots
                        .and_then(|snapshots| snapshots.get(name))
                        .is_some_and(|snapshot| {
                            runtime_usage_snapshot_is_usable(snapshot, now, stale_grace_seconds)
                        });
                    reasons.push(if snapshot_usable {
                        selection_reason(
                            ProfileSelectionReasonKind::NotReady,
                            "profile probe failed and persisted snapshot did not produce a candidate",
                        )
                    } else {
                        selection_reason(
                            ProfileSelectionReasonKind::MissingPersistedSnapshot,
                            "profile probe failed and no usable persisted snapshot was available",
                        )
                    });
                }
            },
        }
    }

    let rank = ranked_candidate.map(|(rank, _)| rank);
    let decision = if selected {
        ProfileSelectionDecision::Selected
    } else if candidate.is_some() {
        ProfileSelectionDecision::Ready
    } else if report.is_none() {
        ProfileSelectionDecision::MissingReport
    } else {
        ProfileSelectionDecision::Rejected
    };

    ProfileSelectionProfileExplanation {
        name: name.to_string(),
        decision,
        rank,
        selected,
        last_run_selected_at,
        candidate,
        blocked_limits,
        probe_error,
        reasons,
    }
}

fn selection_reason(
    kind: ProfileSelectionReasonKind,
    message: impl Into<String>,
) -> ProfileSelectionReason {
    ProfileSelectionReason {
        kind,
        message: message.into(),
    }
}

pub fn render_profile_selection_explanation(explanation: &ProfileSelectionExplanation) -> String {
    let mut lines = Vec::new();
    lines.push(match explanation.selected_profile.as_deref() {
        Some(profile) => format!("Selected profile: {profile}"),
        None => "Selected profile: none".to_string(),
    });
    if let Some(current) = explanation.current_profile.as_deref() {
        lines.push(format!("Current profile: {current}"));
    }
    if let Some(preferred) = explanation.preferred_profile.as_deref() {
        lines.push(format!("Preferred profile: {preferred}"));
    }
    for profile in &explanation.profiles {
        let rank = profile
            .rank
            .map(|rank| format!(" rank={}", rank + 1))
            .unwrap_or_default();
        lines.push(format!(
            "- {}: {:?}{}",
            profile.name, profile.decision, rank
        ));
        for reason in &profile.reasons {
            lines.push(format!("  - {:?}: {}", reason.kind, reason.message));
        }
    }
    lines.join("\n")
}

pub fn schedule_ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    mut candidates: Vec<ReadyProfileCandidate>,
    selection: S,
    preferred_profile: Option<&str>,
) -> Vec<ReadyProfileCandidate> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let now = Local::now().timestamp();
    let best_provider_priority = candidates
        .iter()
        .map(|candidate| candidate.provider_priority)
        .min()
        .unwrap_or(usize::MAX);
    let best_total_pressure = candidates
        .iter()
        .filter(|candidate| candidate.provider_priority == best_provider_priority)
        .map(|candidate| ready_profile_score(candidate).total_pressure)
        .min()
        .unwrap_or(i64::MAX);

    candidates.sort_by_key(|candidate| {
        ready_profile_runtime_sort_key_with_view(
            candidate,
            selection,
            best_provider_priority,
            best_total_pressure,
            now,
        )
    });

    if let Some(preferred_name) = preferred_profile
        && let Some(preferred_index) = candidates.iter().position(|candidate| {
            candidate.name == preferred_name
                && !profile_in_run_selection_cooldown_with_view(selection, &candidate.name, now)
        })
    {
        let preferred_score = ready_profile_score(&candidates[preferred_index]).total_pressure;
        let selected_score = ready_profile_score(&candidates[0]).total_pressure;

        if preferred_index > 0
            && candidates[preferred_index].provider_priority == candidates[0].provider_priority
            && score_within_bps(
                preferred_score,
                selected_score,
                RUN_SELECTION_HYSTERESIS_BPS,
            )
        {
            let preferred_candidate = candidates.remove(preferred_index);
            candidates.insert(0, preferred_candidate);
        }
    }

    candidates
}

pub type ReadyProfileSortKey = (
    usize,
    i64,
    i64,
    i64,
    Reverse<i64>,
    Reverse<i64>,
    Reverse<i64>,
    i64,
    i64,
    usize,
    usize,
    usize,
);

pub type ReadyProfileRuntimeSortKey = (usize, usize, usize, i64, ReadyProfileSortKey);

pub fn ready_profile_runtime_sort_key_with_view<S: ProfileSelectionRead>(
    candidate: &ReadyProfileCandidate,
    selection: S,
    best_provider_priority: usize,
    best_total_pressure: i64,
    now: i64,
) -> ReadyProfileRuntimeSortKey {
    let score = ready_profile_score(candidate);
    let near_optimal = candidate.provider_priority == best_provider_priority
        && score_within_bps(
            score.total_pressure,
            best_total_pressure,
            RUN_SELECTION_NEAR_OPTIMAL_BPS,
        );
    let recently_used = near_optimal
        && profile_in_run_selection_cooldown_with_view(selection, &candidate.name, now);
    let last_selected_at = if near_optimal {
        selection
            .last_run_selected_at(&candidate.name)
            .unwrap_or(i64::MIN)
    } else {
        i64::MIN
    };

    (
        candidate.provider_priority,
        if near_optimal { 0usize } else { 1usize },
        if recently_used { 1usize } else { 0usize },
        last_selected_at,
        ready_profile_sort_key(candidate),
    )
}

pub fn ready_profile_sort_key(candidate: &ReadyProfileCandidate) -> ReadyProfileSortKey {
    let score = ready_profile_score(candidate);

    (
        candidate.provider_priority,
        score.total_pressure,
        score.weekly_pressure,
        score.five_hour_pressure,
        Reverse(score.reserve_floor),
        Reverse(score.weekly_remaining),
        Reverse(score.five_hour_remaining),
        score.weekly_reset_at,
        score.five_hour_reset_at,
        runtime_quota_source_sort_key(RuntimeRouteKind::Responses, candidate.quota_source),
        if candidate.preferred { 0usize } else { 1usize },
        candidate.order_index,
    )
}

pub fn ready_profile_score(candidate: &ReadyProfileCandidate) -> ReadyProfileScore {
    ready_profile_score_for_route(&candidate.usage, RuntimeRouteKind::Responses)
}

pub fn ready_profile_score_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> ReadyProfileScore {
    ready_profile_score_for_route_at(usage, route_kind, Local::now().timestamp())
}

pub fn ready_profile_score_for_route_at(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> ReadyProfileScore {
    let weekly = required_main_window_snapshot_at(usage, "weekly", now);
    let five_hour = required_main_window_snapshot_at(usage, "5h", now);

    let weekly_pressure = weekly.map_or(i64::MAX, |window| window.pressure_score);
    let five_hour_pressure = five_hour.map_or(i64::MAX, |window| window.pressure_score);
    let weekly_remaining = weekly.map_or(0, |window| window.remaining_percent);
    let five_hour_remaining = five_hour.map_or(0, |window| window.remaining_percent);
    let weekly_weight = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => 10,
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => 8,
    };
    let reserve_bias = match runtime_quota_pressure_band_for_route_at(usage, route_kind, now) {
        RuntimeQuotaPressureBand::Healthy => 0,
        RuntimeQuotaPressureBand::Thin => 250_000,
        RuntimeQuotaPressureBand::Critical => 1_000_000,
        RuntimeQuotaPressureBand::Exhausted | RuntimeQuotaPressureBand::Unknown => i64::MAX / 4,
    };

    ReadyProfileScore {
        total_pressure: reserve_bias
            .saturating_add(weekly_pressure.saturating_mul(weekly_weight))
            .saturating_add(five_hour_pressure),
        weekly_pressure,
        five_hour_pressure,
        reserve_floor: weekly_remaining.min(five_hour_remaining),
        weekly_remaining,
        five_hour_remaining,
        weekly_reset_at: weekly.map_or(i64::MAX, |window| window.reset_at),
        five_hour_reset_at: five_hour.map_or(i64::MAX, |window| window.reset_at),
    }
}

pub fn runtime_quota_pressure_band_for_route(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
) -> RuntimeQuotaPressureBand {
    runtime_quota_pressure_band_for_route_at(usage, route_kind, Local::now().timestamp())
}

pub fn runtime_quota_pressure_band_for_route_at(
    usage: &UsageResponse,
    route_kind: RuntimeRouteKind,
    now: i64,
) -> RuntimeQuotaPressureBand {
    let Some(weekly) = required_main_window_snapshot_at(usage, "weekly", now) else {
        return RuntimeQuotaPressureBand::Unknown;
    };
    let Some(five_hour) = required_main_window_snapshot_at(usage, "5h", now) else {
        return RuntimeQuotaPressureBand::Unknown;
    };

    let weekly_remaining = weekly.remaining_percent;
    let five_hour_remaining = five_hour.remaining_percent;
    if weekly_remaining == 0 || five_hour_remaining == 0 {
        return RuntimeQuotaPressureBand::Exhausted;
    }

    let (thin_weekly, thin_five_hour, critical_weekly, critical_five_hour) = match route_kind {
        RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket => (20, 10, 10, 5),
        RuntimeRouteKind::Compact | RuntimeRouteKind::Standard => (10, 5, 5, 3),
    };

    if weekly_remaining <= critical_weekly || five_hour_remaining <= critical_five_hour {
        RuntimeQuotaPressureBand::Critical
    } else if weekly_remaining <= thin_weekly || five_hour_remaining <= thin_five_hour {
        RuntimeQuotaPressureBand::Thin
    } else {
        RuntimeQuotaPressureBand::Healthy
    }
}

pub fn runtime_quota_source_sort_key(
    route_kind: RuntimeRouteKind,
    source: RuntimeQuotaSource,
) -> usize {
    match (route_kind, source) {
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::LiveProbe,
        ) => 0,
        (
            RuntimeRouteKind::Responses | RuntimeRouteKind::Websocket,
            RuntimeQuotaSource::PersistedSnapshot,
        ) => 1,
        _ => 0,
    }
}

pub fn profile_in_run_selection_cooldown_with_view<S: ProfileSelectionRead>(
    selection: S,
    profile_name: &str,
    now: i64,
) -> bool {
    let Some(last_selected_at) = selection.last_run_selected_at(profile_name) else {
        return false;
    };

    now.saturating_sub(last_selected_at) < RUN_SELECTION_COOLDOWN_SECONDS
}

pub fn score_within_bps(candidate_score: i64, best_score: i64, bps: i64) -> bool {
    if candidate_score <= best_score {
        return true;
    }

    let lhs = i128::from(candidate_score).saturating_mul(10_000);
    let rhs = i128::from(best_score).saturating_mul(i128::from(10_000 + bps));
    lhs <= rhs
}

pub fn active_profile_selection_order_with_view<S: ProfileSelectionRead>(
    selection: S,
    current_profile: &str,
) -> Vec<String> {
    provider_aware_profile_order_with_view(
        selection,
        std::iter::once(current_profile.to_string())
            .chain(profile_rotation_order_with_view(selection, current_profile)),
    )
}

pub fn profile_rotation_order_with_view<S: ProfileSelectionRead>(
    selection: S,
    current_profile: &str,
) -> Vec<String> {
    let names = selection.profile_names();
    let Some(index) = names.iter().position(|name| name == current_profile) else {
        return provider_aware_profile_order_with_view(
            selection,
            names.into_iter().filter(|name| name != current_profile),
        );
    };

    provider_aware_profile_order_with_view(
        selection,
        names
            .iter()
            .skip(index + 1)
            .chain(names.iter().take(index))
            .cloned(),
    )
}

pub fn provider_aware_profile_order_with_view<S: ProfileSelectionRead, I>(
    selection: S,
    names: I,
) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut ordered = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let provider_priority = selection
                .profile_entry(&name)
                .map(ProfileSelectionProvider::runtime_pool_priority)
                .unwrap_or(usize::MAX);
            (provider_priority, index, name)
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(provider_priority, index, _)| (*provider_priority, *index));
    ordered.into_iter().map(|(_, _, name)| name).collect()
}

#[cfg(test)]
#[path = "../tests/src/selection.rs"]
mod tests;
