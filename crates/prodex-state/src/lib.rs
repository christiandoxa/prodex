use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

mod provider_capabilities;
pub use provider_capabilities::{ProviderCapabilities, ProviderQuotaShape, RuntimeRoutePolicy};

pub const SESSION_ID_PROFILE_BINDING_LIMIT: usize = if cfg!(test) { 64 } else { 2_048 };
pub const APP_STATE_LAST_RUN_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 90 * 24 * 60 * 60 };
pub const APP_STATE_SESSION_BINDING_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 30 * 24 * 60 * 60 };
pub const PROFILE_GOVERNANCE_DEFAULT_WEIGHT: i64 = 100;
pub const PROFILE_GOVERNANCE_MIN_WEIGHT: i64 = 0;
pub const PROFILE_GOVERNANCE_MAX_WEIGHT: i64 = 10_000;
pub const PROFILE_GOVERNANCE_MAX_TAGS: usize = 32;
pub const PROFILE_GOVERNANCE_MAX_TAG_LENGTH: usize = 48;
pub const PROFILE_GOVERNANCE_MAX_NOTE_LENGTH: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppStateCompactionPolicy {
    pub last_run_retention_seconds: i64,
    pub session_binding_retention_seconds: i64,
    pub session_binding_limit: usize,
}

impl Default for AppStateCompactionPolicy {
    fn default() -> Self {
        Self {
            last_run_retention_seconds: APP_STATE_LAST_RUN_RETENTION_SECONDS,
            session_binding_retention_seconds: APP_STATE_SESSION_BINDING_RETENTION_SECONDS,
            session_binding_limit: SESSION_ID_PROFILE_BINDING_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppState {
    pub active_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileEntry>,
    #[serde(default)]
    pub last_run_selected_at: BTreeMap<String, i64>,
    #[serde(default)]
    pub response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    pub session_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileEntry {
    pub codex_home: PathBuf,
    pub managed: bool,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub provider: ProfileProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "provider_kind", rename_all = "snake_case")]
pub enum ProfileProvider {
    #[default]
    Openai,
    Gemini {
        email: String,
        #[serde(default)]
        project_id: Option<String>,
    },
    Anthropic {
        #[serde(default)]
        account: Option<String>,
        #[serde(default)]
        auth_method: Option<String>,
    },
    Copilot {
        host: String,
        login: String,
        api_url: String,
        #[serde(default)]
        access_type_sku: Option<String>,
        #[serde(default)]
        copilot_plan: Option<String>,
    },
    Agy {
        #[serde(default)]
        account: Option<String>,
    },
}

impl ProfileProvider {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Gemini { .. } => "gemini",
            Self::Anthropic { .. } => "anthropic",
            Self::Copilot { .. } => "copilot",
            Self::Agy { .. } => "agy",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Openai => "OpenAI/Codex",
            Self::Gemini { .. } => "Google Gemini",
            Self::Anthropic { .. } => "Anthropic Claude",
            Self::Copilot { .. } => "GitHub Copilot",
            Self::Agy { .. } => "Anti-Gravity",
        }
    }

    pub fn runtime_pool_priority(&self) -> usize {
        match self {
            // Native OpenAI/Codex pool stays primary; other providers are fallback candidates.
            Self::Openai => 0,
            _ => 1,
        }
    }

    pub fn copilot_matches(&self, host: &str, login: &str) -> bool {
        match self {
            Self::Copilot {
                host: stored_host,
                login: stored_login,
                ..
            } => stored_host.trim() == host.trim() && stored_login.trim() == login.trim(),
            Self::Openai | Self::Gemini { .. } | Self::Anthropic { .. } | Self::Agy { .. } => false,
        }
    }

    pub fn gemini_matches(&self, email: &str) -> bool {
        match self {
            Self::Gemini {
                email: stored_email,
                ..
            } => stored_email.trim().eq_ignore_ascii_case(email.trim()),
            Self::Openai | Self::Anthropic { .. } | Self::Copilot { .. } | Self::Agy { .. } => {
                false
            }
        }
    }

    pub fn anthropic_matches(&self, account: Option<&str>, auth_method: Option<&str>) -> bool {
        match self {
            Self::Anthropic {
                account: stored_account,
                auth_method: stored_auth_method,
            } => {
                let account_matches = match (stored_account.as_deref(), account) {
                    (Some(left), Some(right)) => left.trim().eq_ignore_ascii_case(right.trim()),
                    (None, None) => true,
                    _ => false,
                };
                let auth_method_matches = match (stored_auth_method.as_deref(), auth_method) {
                    (Some(left), Some(right)) => left.trim().eq_ignore_ascii_case(right.trim()),
                    (None, _) => true,
                    (_, None) => true,
                };
                account_matches && auth_method_matches
            }
            Self::Openai | Self::Gemini { .. } | Self::Copilot { .. } | Self::Agy { .. } => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponseProfileBinding {
    pub profile_name: String,
    pub bound_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileGovernancePolicy {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_profile_governance_weight")]
    pub weight: i64,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub drained: bool,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub updated_at: i64,
}

impl Default for ProfileGovernancePolicy {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            weight: PROFILE_GOVERNANCE_DEFAULT_WEIGHT,
            paused: false,
            drained: false,
            note: None,
            updated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileSelectionEligibilityReason {
    Eligible,
    MissingProfile,
    Paused,
    Drained,
}

impl ProfileSelectionEligibilityReason {
    pub fn is_eligible(self) -> bool {
        matches!(self, Self::Eligible)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::MissingProfile => "missing_profile",
            Self::Paused => "paused",
            Self::Drained => "drained",
        }
    }
}

fn default_profile_governance_weight() -> i64 {
    PROFILE_GOVERNANCE_DEFAULT_WEIGHT
}

pub fn normalize_profile_governance_tag(tag: &str) -> Option<String> {
    let normalized = tag
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .take(PROFILE_GOVERNANCE_MAX_TAG_LENGTH)
        .collect::<String>();
    (!normalized.is_empty()).then_some(normalized)
}

pub fn normalize_profile_governance_tags<I, S>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized = Vec::new();
    for tag in tags {
        let Some(tag) = normalize_profile_governance_tag(tag.as_ref()) else {
            continue;
        };
        if normalized.contains(&tag) {
            continue;
        }
        normalized.push(tag);
        if normalized.len() >= PROFILE_GOVERNANCE_MAX_TAGS {
            break;
        }
    }
    normalized
}

pub fn clamp_profile_governance_weight(weight: i64) -> i64 {
    weight.clamp(PROFILE_GOVERNANCE_MIN_WEIGHT, PROFILE_GOVERNANCE_MAX_WEIGHT)
}

pub fn normalize_profile_governance_note(note: Option<String>) -> Option<String> {
    let note = note?;
    let trimmed = note.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .chars()
            .take(PROFILE_GOVERNANCE_MAX_NOTE_LENGTH)
            .collect(),
    )
}

pub fn normalize_profile_governance_policy(
    mut policy: ProfileGovernancePolicy,
) -> ProfileGovernancePolicy {
    policy.tags = normalize_profile_governance_tags(policy.tags);
    policy.weight = clamp_profile_governance_weight(policy.weight);
    policy.note = normalize_profile_governance_note(policy.note);
    policy
}

pub fn upsert_profile_governance_policy(
    policies: &mut BTreeMap<String, ProfileGovernancePolicy>,
    profile_name: impl Into<String>,
    policy: ProfileGovernancePolicy,
) -> Option<ProfileGovernancePolicy> {
    policies.insert(
        profile_name.into(),
        normalize_profile_governance_policy(policy),
    )
}

pub fn mutate_profile_governance_policy<F>(
    policies: &mut BTreeMap<String, ProfileGovernancePolicy>,
    profile_name: impl Into<String>,
    updated_at: i64,
    mutate: F,
) where
    F: FnOnce(&mut ProfileGovernancePolicy),
{
    let profile_name = profile_name.into();
    let mut policy = policies.remove(&profile_name).unwrap_or_default();
    mutate(&mut policy);
    policy.updated_at = updated_at;
    policies.insert(profile_name, normalize_profile_governance_policy(policy));
}

pub fn prune_profile_governance_policies(
    policies: &mut BTreeMap<String, ProfileGovernancePolicy>,
    profiles: &BTreeMap<String, ProfileEntry>,
) {
    policies.retain(|profile_name, _| profiles.contains_key(profile_name));
}

pub fn merge_profile_governance_policies(
    existing: &BTreeMap<String, ProfileGovernancePolicy>,
    incoming: &BTreeMap<String, ProfileGovernancePolicy>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, ProfileGovernancePolicy> {
    let mut merged = existing.clone();
    for (profile_name, policy) in incoming {
        let should_replace = merged
            .get(profile_name)
            .is_none_or(|current| current.updated_at <= policy.updated_at);
        if should_replace {
            merged.insert(
                profile_name.clone(),
                normalize_profile_governance_policy(policy.clone()),
            );
        }
    }
    prune_profile_governance_policies(&mut merged, profiles);
    merged
}

pub fn profile_selection_eligibility_reason(
    profiles: &BTreeMap<String, ProfileEntry>,
    policies: &BTreeMap<String, ProfileGovernancePolicy>,
    profile_name: &str,
) -> ProfileSelectionEligibilityReason {
    if !profiles.contains_key(profile_name) {
        return ProfileSelectionEligibilityReason::MissingProfile;
    }
    let Some(policy) = policies.get(profile_name) else {
        return ProfileSelectionEligibilityReason::Eligible;
    };
    if policy.paused {
        return ProfileSelectionEligibilityReason::Paused;
    }
    if policy.drained {
        return ProfileSelectionEligibilityReason::Drained;
    }
    ProfileSelectionEligibilityReason::Eligible
}

pub fn merge_last_run_selection(
    existing: &BTreeMap<String, i64>,
    incoming: &BTreeMap<String, i64>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, i64> {
    let mut merged = existing.clone();
    for (profile_name, timestamp) in incoming {
        merged
            .entry(profile_name.clone())
            .and_modify(|current| *current = (*current).max(*timestamp))
            .or_insert(*timestamp);
    }
    merged.retain(|profile_name, _| profiles.contains_key(profile_name));
    merged
}

pub fn prune_last_run_selection(
    selections: &mut BTreeMap<String, i64>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
) {
    prune_last_run_selection_with_retention(
        selections,
        profiles,
        now,
        APP_STATE_LAST_RUN_RETENTION_SECONDS,
    );
}

pub fn prune_last_run_selection_with_retention(
    selections: &mut BTreeMap<String, i64>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    retention_seconds: i64,
) {
    let oldest_allowed = now.saturating_sub(retention_seconds);
    selections.retain(|profile_name, timestamp| {
        profiles.contains_key(profile_name) && *timestamp >= oldest_allowed
    });
}

pub fn merge_profile_bindings(
    existing: &BTreeMap<String, ResponseProfileBinding>,
    incoming: &BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> BTreeMap<String, ResponseProfileBinding> {
    let mut merged = existing.clone();
    for (response_id, binding) in incoming {
        let should_replace = merged
            .get(response_id)
            .is_none_or(|current| current.bound_at <= binding.bound_at);
        if should_replace {
            merged.insert(response_id.clone(), binding.clone());
        }
    }
    merged.retain(|_, binding| profiles.contains_key(&binding.profile_name));
    merged
}

pub fn remap_profile_binding_targets(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    from_profile: &str,
    to_profile: &str,
) {
    if from_profile == to_profile {
        return;
    }
    for binding in bindings.values_mut() {
        if binding.profile_name == from_profile {
            binding.profile_name = to_profile.to_string();
        }
    }
}

pub fn duplicate_profile_identity_key(
    account_id: Option<&str>,
    email: Option<&str>,
) -> Option<String> {
    let account_id = account_id
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty());
    let email = email
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_ascii_lowercase);

    match (account_id, email) {
        (Some(account_id), Some(email)) => Some(format!("account:{account_id}|email:{email}")),
        (Some(account_id), None) => Some(format!("account:{account_id}")),
        (None, Some(email)) => Some(format!("email:{email}")),
        (None, None) => None,
    }
}

pub fn select_canonical_duplicate_profile(
    state: &AppState,
    profile_names: &[String],
) -> Option<String> {
    profile_names.iter().cloned().max_by(|left, right| {
        duplicate_profile_cleanup_priority(state, left)
            .cmp(&duplicate_profile_cleanup_priority(state, right))
            .then_with(|| right.cmp(left))
    })
}

pub fn remove_duplicate_profile_from_state(
    state: &mut AppState,
    duplicate_name: &str,
    canonical_name: &str,
) -> Option<ProfileEntry> {
    let duplicate_last_selected_at = state.last_run_selected_at.remove(duplicate_name);
    if let Some(last_selected_at) = duplicate_last_selected_at {
        let target = state
            .last_run_selected_at
            .entry(canonical_name.to_string())
            .or_insert(last_selected_at);
        *target = (*target).max(last_selected_at);
    }

    remap_profile_binding_targets(
        &mut state.response_profile_bindings,
        duplicate_name,
        canonical_name,
    );
    remap_profile_binding_targets(
        &mut state.session_profile_bindings,
        duplicate_name,
        canonical_name,
    );

    if state.active_profile.as_deref() == Some(duplicate_name) {
        state.active_profile = Some(canonical_name.to_string());
    }

    state.profiles.remove(duplicate_name)
}

pub fn ensure_active_profile_after_duplicate_cleanup(state: &mut AppState) {
    if state.active_profile.is_none() {
        state.active_profile = state.profiles.keys().next().cloned();
    }
}

fn duplicate_profile_cleanup_priority(state: &AppState, profile_name: &str) -> (bool, i64, bool) {
    let active = state.active_profile.as_deref() == Some(profile_name);
    let last_selected_at = state
        .last_run_selected_at
        .get(profile_name)
        .copied()
        .unwrap_or(i64::MIN);
    let prefer_external = state
        .profiles
        .get(profile_name)
        .map(|profile| !profile.managed)
        .unwrap_or(false);
    (active, last_selected_at, prefer_external)
}

pub fn prune_profile_bindings(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    max_entries: usize,
) {
    if bindings.len() <= max_entries {
        return;
    }

    let excess = bindings.len() - max_entries;
    let mut oldest = bindings
        .iter()
        .map(|(response_id, binding)| (response_id.clone(), binding.bound_at))
        .collect::<Vec<_>>();
    oldest.sort_by_key(|(_, bound_at)| *bound_at);

    for (response_id, _) in oldest.into_iter().take(excess) {
        bindings.remove(&response_id);
    }
}

pub fn prune_profile_bindings_for_housekeeping(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
    now: i64,
    retention_seconds: i64,
    max_entries: usize,
) {
    let oldest_allowed = now.saturating_sub(retention_seconds);
    bindings.retain(|_, binding| {
        profiles.contains_key(&binding.profile_name) && binding.bound_at >= oldest_allowed
    });
    prune_profile_bindings(bindings, max_entries);
}

pub fn prune_profile_bindings_for_housekeeping_without_retention(
    bindings: &mut BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) {
    bindings.retain(|_, binding| profiles.contains_key(&binding.profile_name));
}

pub fn compact_app_state(state: AppState, now: i64) -> AppState {
    compact_app_state_with_policy(state, now, AppStateCompactionPolicy::default())
}

pub fn compact_app_state_with_policy(
    mut state: AppState,
    now: i64,
    policy: AppStateCompactionPolicy,
) -> AppState {
    state.active_profile = state
        .active_profile
        .filter(|profile_name| state.profiles.contains_key(profile_name));
    prune_last_run_selection_with_retention(
        &mut state.last_run_selected_at,
        &state.profiles,
        now,
        policy.last_run_retention_seconds,
    );
    prune_profile_bindings_for_housekeeping_without_retention(
        &mut state.response_profile_bindings,
        &state.profiles,
    );
    prune_profile_bindings_for_housekeeping(
        &mut state.session_profile_bindings,
        &state.profiles,
        now,
        policy.session_binding_retention_seconds,
        policy.session_binding_limit,
    );
    state
}

pub fn merge_app_state_for_save(existing: AppState, desired: &AppState) -> AppState {
    merge_app_state_for_save_with_policy(
        existing,
        desired,
        current_unix_timestamp(),
        AppStateCompactionPolicy::default(),
    )
}

pub fn merge_app_state_for_save_with_policy(
    existing: AppState,
    desired: &AppState,
    now: i64,
    policy: AppStateCompactionPolicy,
) -> AppState {
    let active_profile = desired
        .active_profile
        .clone()
        .filter(|profile_name| desired.profiles.contains_key(profile_name));
    let merged = AppState {
        active_profile,
        profiles: desired.profiles.clone(),
        last_run_selected_at: merge_last_run_selection(
            &existing.last_run_selected_at,
            &desired.last_run_selected_at,
            &desired.profiles,
        ),
        response_profile_bindings: merge_profile_bindings(
            &existing.response_profile_bindings,
            &desired.response_profile_bindings,
            &desired.profiles,
        ),
        session_profile_bindings: merge_profile_bindings(
            &existing.session_profile_bindings,
            &desired.session_profile_bindings,
            &desired.profiles,
        ),
    };
    compact_app_state_with_policy(merged, now, policy)
}

pub fn merge_runtime_state_snapshot(existing: AppState, snapshot: &AppState) -> AppState {
    merge_runtime_state_snapshot_with_policy(
        existing,
        snapshot,
        current_unix_timestamp(),
        AppStateCompactionPolicy::default(),
    )
}

pub fn merge_runtime_state_snapshot_with_policy(
    existing: AppState,
    snapshot: &AppState,
    now: i64,
    policy: AppStateCompactionPolicy,
) -> AppState {
    let profiles = if existing.profiles.is_empty() {
        snapshot.profiles.clone()
    } else {
        existing.profiles.clone()
    };
    let active_profile = snapshot
        .active_profile
        .clone()
        .or(existing.active_profile.clone())
        .filter(|profile_name| profiles.contains_key(profile_name));

    let merged = AppState {
        active_profile,
        profiles: profiles.clone(),
        last_run_selected_at: merge_last_run_selection(
            &existing.last_run_selected_at,
            &snapshot.last_run_selected_at,
            &profiles,
        ),
        response_profile_bindings: merge_profile_bindings(
            &existing.response_profile_bindings,
            &snapshot.response_profile_bindings,
            &profiles,
        ),
        session_profile_bindings: merge_profile_bindings(
            &existing.session_profile_bindings,
            &snapshot.session_profile_bindings,
            &profiles,
        ),
    };
    compact_app_state_with_policy(merged, now, policy)
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
