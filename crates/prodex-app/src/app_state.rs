use anyhow::{Context, Result};
use chrono::Local;
use std::path::Path;

use crate::{
    AppPaths, AuthSummary, RecoveredLoad, acquire_state_file_lock, compact_app_state,
    gemini_oauth_secret_path, load_json_file_with_backup, merge_app_state_for_save,
    read_auth_summary, state_last_good_file_path, write_state_json_atomic,
};

pub(crate) use prodex_state::{
    AppState, ProfileEntry, ProfileProvider, ResponseProfileBinding, prune_profile_bindings,
};

pub(crate) trait ProfileProviderExt {
    fn auth_summary(&self, codex_home: &Path) -> AuthSummary;
}

impl ProfileProviderExt for ProfileProvider {
    fn auth_summary(&self, codex_home: &Path) -> AuthSummary {
        match self {
            Self::Openai => read_auth_summary(codex_home),
            Self::Gemini { .. } => AuthSummary {
                label: "gemini-oauth".to_string(),
                quota_compatible: gemini_oauth_secret_path(codex_home).exists(),
            },
            Self::Anthropic { .. } => AuthSummary {
                label: "claude-oauth".to_string(),
                quota_compatible: false,
            },
            Self::Copilot { .. } => AuthSummary {
                label: "copilot".to_string(),
                quota_compatible: false,
            },
            Self::Kiro { .. } => AuthSummary {
                label: "kiro".to_string(),
                quota_compatible: false,
            },
            Self::Agy { .. } => AuthSummary {
                label: "agy".to_string(),
                quota_compatible: false,
            },
        }
    }
}

pub(crate) trait AppStateIoExt: Sized {
    fn load_with_recovery(paths: &AppPaths) -> Result<RecoveredLoad<Self>>;
    fn load(paths: &AppPaths) -> Result<Self>;
    fn load_and_repair(paths: &AppPaths) -> Result<Self>;
    fn save(&self, paths: &AppPaths) -> Result<()>;
}

impl AppStateIoExt for AppState {
    fn load_with_recovery(paths: &AppPaths) -> Result<RecoveredLoad<Self>> {
        if !paths.state_file.exists() && !state_last_good_file_path(paths).exists() {
            return Ok(RecoveredLoad {
                value: Self::default(),
                recovered_from_backup: false,
            });
        }

        let loaded = load_json_file_with_backup::<Self>(
            &paths.state_file,
            &state_last_good_file_path(paths),
        )?;
        Ok(RecoveredLoad {
            value: compact_app_state(loaded.value, Local::now().timestamp()),
            recovered_from_backup: loaded.recovered_from_backup,
        })
    }

    fn load(paths: &AppPaths) -> Result<Self> {
        Ok(Self::load_with_recovery(paths)?.value)
    }

    fn load_and_repair(paths: &AppPaths) -> Result<Self> {
        if !paths.state_file.exists() && !state_last_good_file_path(paths).exists() {
            return Ok(Self::default());
        }

        let _lock = acquire_state_file_lock(paths)?;
        let loaded = load_json_file_with_backup::<Self>(
            &paths.state_file,
            &state_last_good_file_path(paths),
        )?;
        let compacted = compact_app_state(loaded.value.clone(), Local::now().timestamp());
        if compacted != loaded.value {
            let json = serde_json::to_string_pretty(&compacted)
                .context("failed to serialize prodex state")?;
            write_state_json_atomic(paths, &json)?;
        }
        Ok(compacted)
    }

    fn save(&self, paths: &AppPaths) -> Result<()> {
        let _lock = acquire_state_file_lock(paths)?;
        let existing = Self::load(paths)?;
        let merged = compact_app_state(
            merge_app_state_for_save(existing, self),
            Local::now().timestamp(),
        );
        let json =
            serde_json::to_string_pretty(&merged).context("failed to serialize prodex state")?;
        write_state_json_atomic(paths, &json)?;
        Ok(())
    }
}

pub(crate) fn repair_missing_active_profile(state: &mut AppState) -> Option<String> {
    if state
        .active_profile
        .as_deref()
        .is_some_and(|profile_name| state.profiles.contains_key(profile_name))
    {
        return None;
    }

    let selected = state.profiles.keys().next().cloned()?;
    state.active_profile = Some(selected.clone());
    Some(selected)
}

pub(crate) fn repair_missing_active_profile_and_save(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<Option<String>> {
    let repaired = repair_missing_active_profile(state);
    if repaired.is_some() {
        state.save(paths)?;
    }
    Ok(repaired)
}
