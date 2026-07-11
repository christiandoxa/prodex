use crate::info::{
    RuntimeProfileUsageSnapshot, runtime_usage_snapshot_is_usable,
    usage_from_runtime_usage_snapshot,
};
use chrono::Local;
use prodex_quota::collect_blocked_limits;
use prodex_shared_types::{
    ReadyProfileCandidate, ReadyProfileScore, RunProfileProbeReport, RuntimeProfileProbeCacheEntry,
    RuntimeQuotaSource,
};
use prodex_state::{ProfileEntry, ProfileProvider};
use serde::{Deserialize, Serialize};
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

pub struct ReadyProfileCandidatesExplanationRequest<'a, S> {
    pub reports: &'a [RunProfileProbeReport],
    pub scheduled_candidates: &'a [ReadyProfileCandidate],
    pub include_code_review: bool,
    pub current_profile: Option<&'a str>,
    pub preferred_profile: Option<&'a str>,
    pub selection: S,
    pub persisted_usage_snapshots: Option<&'a BTreeMap<String, RuntimeProfileUsageSnapshot>>,
    pub stale_grace_seconds: i64,
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
    explain_ready_profile_candidates_with_view(ReadyProfileCandidatesExplanationRequest {
        reports,
        scheduled_candidates: &candidates,
        include_code_review,
        current_profile,
        preferred_profile,
        selection,
        persisted_usage_snapshots,
        stale_grace_seconds,
    })
}

pub fn explain_ready_profile_candidates_with_view<S: ProfileSelectionRead>(
    request: ReadyProfileCandidatesExplanationRequest<'_, S>,
) -> ProfileSelectionExplanation {
    let ReadyProfileCandidatesExplanationRequest {
        reports,
        scheduled_candidates,
        include_code_review,
        current_profile,
        preferred_profile,
        selection,
        persisted_usage_snapshots,
        stale_grace_seconds,
    } = request;
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
            let context = ProfileSelectionEntryExplanationContext {
                include_code_review,
                current_profile,
                preferred_profile,
                selection,
                persisted_usage_snapshots,
                stale_grace_seconds,
                now,
                selected_profile: selected_profile.as_deref(),
            };
            explain_profile_selection_entry(
                &name,
                report_by_name.get(name.as_str()).copied(),
                candidate_ranks.get(name.as_str()).copied(),
                context,
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
    explain_ready_profile_candidates_with_view(ReadyProfileCandidatesExplanationRequest {
        reports: &[],
        scheduled_candidates: &[],
        include_code_review: false,
        current_profile,
        preferred_profile,
        selection: RuntimeProfileSelectionCatalogView {
            entries: &catalog.entries,
        },
        persisted_usage_snapshots: None,
        stale_grace_seconds: 0,
    })
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

#[derive(Clone, Copy)]
struct ProfileSelectionEntryExplanationContext<'a, S> {
    include_code_review: bool,
    current_profile: Option<&'a str>,
    preferred_profile: Option<&'a str>,
    selection: S,
    persisted_usage_snapshots: Option<&'a BTreeMap<String, RuntimeProfileUsageSnapshot>>,
    stale_grace_seconds: i64,
    now: i64,
    selected_profile: Option<&'a str>,
}

fn explain_profile_selection_entry<S: ProfileSelectionRead>(
    name: &str,
    report: Option<&RunProfileProbeReport>,
    ranked_candidate: Option<(usize, &ReadyProfileCandidate)>,
    context: ProfileSelectionEntryExplanationContext<'_, S>,
) -> ProfileSelectionProfileExplanation {
    let ProfileSelectionEntryExplanationContext {
        include_code_review,
        current_profile,
        preferred_profile,
        selection,
        persisted_usage_snapshots,
        stale_grace_seconds,
        now,
        selected_profile,
    } = context;
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

#[cfg(test)]
#[path = "../tests/src/selection.rs"]
mod tests;
