use super::*;

pub(crate) use prodex_state::{AppState, ProfileEntry, ProfileProvider, ResponseProfileBinding};

pub(crate) trait ProfileProviderExt {
    fn auth_summary(&self, codex_home: &Path) -> AuthSummary;
}

impl ProfileProviderExt for ProfileProvider {
    fn auth_summary(&self, codex_home: &Path) -> AuthSummary {
        match self {
            Self::Openai => read_auth_summary(codex_home),
            Self::Copilot { .. } => AuthSummary {
                label: "copilot".to_string(),
                quota_compatible: false,
            },
        }
    }
}

pub(crate) trait AppStateIoExt: Sized {
    fn load_with_recovery(paths: &AppPaths) -> Result<RecoveredLoad<Self>>;
    fn load(paths: &AppPaths) -> Result<Self>;
    fn save(&self, paths: &AppPaths) -> Result<()>;
}

impl AppStateIoExt for AppState {
    fn load_with_recovery(paths: &AppPaths) -> Result<RecoveredLoad<Self>> {
        cleanup_stale_login_dirs(paths);
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

    fn save(&self, paths: &AppPaths) -> Result<()> {
        cleanup_stale_login_dirs(paths);
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
