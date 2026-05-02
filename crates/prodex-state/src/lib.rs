use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const SESSION_ID_PROFILE_BINDING_LIMIT: usize = if cfg!(test) { 64 } else { 2_048 };
pub const APP_STATE_LAST_RUN_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 90 * 24 * 60 * 60 };
pub const APP_STATE_SESSION_BINDING_RETENTION_SECONDS: i64 =
    if cfg!(test) { 60 } else { 30 * 24 * 60 * 60 };

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
    Copilot {
        host: String,
        login: String,
        api_url: String,
        #[serde(default)]
        access_type_sku: Option<String>,
        #[serde(default)]
        copilot_plan: Option<String>,
    },
}

impl ProfileProvider {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Copilot { .. } => "copilot",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Openai => "OpenAI/Codex",
            Self::Copilot { .. } => "GitHub Copilot",
        }
    }

    pub fn runtime_pool_priority(&self) -> usize {
        match self {
            // Native OpenAI/Codex pool stays primary; other providers are fallback candidates.
            Self::Openai => 0,
            Self::Copilot { .. } => 1,
        }
    }

    pub fn supports_codex_runtime(&self) -> bool {
        matches!(self, Self::Openai)
    }

    pub fn copilot_matches(&self, host: &str, login: &str) -> bool {
        match self {
            Self::Copilot {
                host: stored_host,
                login: stored_login,
                ..
            } => stored_host.trim() == host.trim() && stored_login.trim() == login.trim(),
            Self::Openai => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponseProfileBinding {
    pub profile_name: String,
    pub bound_at: i64,
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
    account_id
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(|account_id| format!("account:{account_id}"))
        .or_else(|| {
            email
                .map(str::trim)
                .filter(|email| !email.is_empty())
                .map(|email| format!("email:{}", email.to_ascii_lowercase()))
        })
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
mod tests {
    use super::*;

    fn profile(name: &str) -> (String, ProfileEntry) {
        (
            name.to_string(),
            ProfileEntry {
                codex_home: PathBuf::from(format!("/tmp/{name}")),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        )
    }

    fn binding(profile_name: &str, bound_at: i64) -> ResponseProfileBinding {
        ResponseProfileBinding {
            profile_name: profile_name.to_string(),
            bound_at,
        }
    }

    fn external_profile(name: &str) -> (String, ProfileEntry) {
        let (name, mut profile) = profile(name);
        profile.managed = false;
        (name, profile)
    }

    #[test]
    fn merge_profile_bindings_prefers_newer_known_profile_binding() {
        let profiles = BTreeMap::from([profile("p1"), profile("p2")]);
        let existing = BTreeMap::from([
            ("same".to_string(), binding("p1", 10)),
            ("old_only".to_string(), binding("p1", 20)),
        ]);
        let incoming = BTreeMap::from([
            ("same".to_string(), binding("p2", 30)),
            ("stale".to_string(), binding("missing", 40)),
        ]);

        let merged = merge_profile_bindings(&existing, &incoming, &profiles);

        assert_eq!(merged.get("same"), Some(&binding("p2", 30)));
        assert_eq!(merged.get("old_only"), Some(&binding("p1", 20)));
        assert!(!merged.contains_key("stale"));
    }

    #[test]
    fn duplicate_identity_key_prefers_account_then_normalized_email() {
        assert_eq!(
            duplicate_profile_identity_key(Some(" acct "), Some("User@Example.COM")),
            Some("account:acct".to_string())
        );
        assert_eq!(
            duplicate_profile_identity_key(None, Some(" User@Example.COM ")),
            Some("email:user@example.com".to_string())
        );
        assert_eq!(duplicate_profile_identity_key(Some(" "), Some(" ")), None);
    }

    #[test]
    fn canonical_duplicate_profile_prefers_active_recent_external_then_name() {
        let state = AppState {
            active_profile: Some("active".to_string()),
            profiles: BTreeMap::from([
                profile("active"),
                profile("recent"),
                external_profile("external"),
                profile("alpha"),
                profile("beta"),
            ]),
            last_run_selected_at: BTreeMap::from([
                ("active".to_string(), 1),
                ("recent".to_string(), 20),
                ("external".to_string(), 20),
                ("alpha".to_string(), 5),
                ("beta".to_string(), 5),
            ]),
            ..AppState::default()
        };

        assert_eq!(
            select_canonical_duplicate_profile(
                &state,
                &[
                    "recent".to_string(),
                    "external".to_string(),
                    "active".to_string()
                ],
            ),
            Some("active".to_string())
        );
        assert_eq!(
            select_canonical_duplicate_profile(
                &state,
                &["recent".to_string(), "external".to_string()],
            ),
            Some("external".to_string())
        );
        assert_eq!(
            select_canonical_duplicate_profile(&state, &["beta".to_string(), "alpha".to_string()]),
            Some("alpha".to_string())
        );
    }

    #[test]
    fn remove_duplicate_profile_from_state_merges_selection_and_remaps_bindings() {
        let mut state = AppState {
            active_profile: Some("duplicate".to_string()),
            profiles: BTreeMap::from([profile("canonical"), profile("duplicate")]),
            last_run_selected_at: BTreeMap::from([
                ("canonical".to_string(), 10),
                ("duplicate".to_string(), 20),
            ]),
            response_profile_bindings: BTreeMap::from([("r".to_string(), binding("duplicate", 1))]),
            session_profile_bindings: BTreeMap::from([("s".to_string(), binding("duplicate", 2))]),
        };

        let removed = remove_duplicate_profile_from_state(&mut state, "duplicate", "canonical");

        assert!(removed.is_some());
        assert_eq!(state.active_profile.as_deref(), Some("canonical"));
        assert!(!state.profiles.contains_key("duplicate"));
        assert_eq!(state.last_run_selected_at.get("canonical"), Some(&20));
        assert_eq!(
            state.response_profile_bindings["r"].profile_name,
            "canonical"
        );
        assert_eq!(
            state.session_profile_bindings["s"].profile_name,
            "canonical"
        );
    }

    #[test]
    fn compact_app_state_prunes_missing_profiles_and_expired_sessions() {
        let now = 120;
        let mut state = AppState {
            active_profile: Some("missing".to_string()),
            profiles: BTreeMap::from([profile("p1")]),
            last_run_selected_at: BTreeMap::from([
                ("p1".to_string(), now),
                ("missing".to_string(), now),
                (
                    "old".to_string(),
                    now - APP_STATE_LAST_RUN_RETENTION_SECONDS - 1,
                ),
            ]),
            response_profile_bindings: BTreeMap::from([
                ("live".to_string(), binding("p1", 1)),
                ("orphan".to_string(), binding("missing", 1)),
            ]),
            session_profile_bindings: BTreeMap::from([
                ("fresh".to_string(), binding("p1", now)),
                (
                    "expired".to_string(),
                    binding("p1", now - APP_STATE_SESSION_BINDING_RETENTION_SECONDS - 1),
                ),
            ]),
        };

        state = compact_app_state(state, now);

        assert_eq!(state.active_profile, None);
        assert_eq!(
            state.last_run_selected_at,
            BTreeMap::from([("p1".to_string(), now)])
        );
        assert_eq!(
            state.response_profile_bindings,
            BTreeMap::from([("live".to_string(), binding("p1", 1))])
        );
        assert_eq!(
            state.session_profile_bindings,
            BTreeMap::from([("fresh".to_string(), binding("p1", now))])
        );
    }

    #[test]
    fn merge_runtime_state_snapshot_keeps_existing_profile_set_when_present() {
        let existing = AppState {
            active_profile: Some("p1".to_string()),
            profiles: BTreeMap::from([profile("p1")]),
            last_run_selected_at: BTreeMap::from([("p1".to_string(), 10)]),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        };
        let snapshot = AppState {
            active_profile: Some("p2".to_string()),
            profiles: BTreeMap::from([profile("p2")]),
            last_run_selected_at: BTreeMap::from([("p2".to_string(), 20)]),
            response_profile_bindings: BTreeMap::from([("r".to_string(), binding("p2", 20))]),
            session_profile_bindings: BTreeMap::new(),
        };

        let merged = merge_runtime_state_snapshot_with_policy(
            existing,
            &snapshot,
            20,
            AppStateCompactionPolicy::default(),
        );

        assert_eq!(merged.active_profile, None);
        assert!(merged.profiles.contains_key("p1"));
        assert!(!merged.profiles.contains_key("p2"));
        assert_eq!(
            merged.last_run_selected_at,
            BTreeMap::from([("p1".to_string(), 10)])
        );
        assert!(merged.response_profile_bindings.is_empty());
    }
}
