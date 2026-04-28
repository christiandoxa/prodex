use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct AppState {
    pub(crate) active_profile: Option<String>,
    #[serde(default)]
    pub(crate) profiles: BTreeMap<String, ProfileEntry>,
    #[serde(default)]
    pub(crate) last_run_selected_at: BTreeMap<String, i64>,
    #[serde(default)]
    pub(crate) response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    pub(crate) session_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProfileEntry {
    pub(crate) codex_home: PathBuf,
    pub(crate) managed: bool,
    #[serde(default)]
    pub(crate) email: Option<String>,
    #[serde(default)]
    pub(crate) provider: ProfileProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "provider_kind", rename_all = "snake_case")]
pub(crate) enum ProfileProvider {
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
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Copilot { .. } => "copilot",
        }
    }

    pub(crate) fn display_name(&self) -> &'static str {
        match self {
            Self::Openai => "OpenAI/Codex",
            Self::Copilot { .. } => "GitHub Copilot",
        }
    }

    pub(crate) fn runtime_pool_priority(&self) -> usize {
        match self {
            // Keep the native OpenAI/Codex pool as the primary family. Other providers are
            // eligible only after the native pool no longer has a viable fresh candidate.
            Self::Openai => 0,
            Self::Copilot { .. } => 1,
        }
    }

    pub(crate) fn supports_codex_runtime(&self) -> bool {
        matches!(self, Self::Openai)
    }

    pub(crate) fn auth_summary(&self, codex_home: &Path) -> AuthSummary {
        match self {
            Self::Openai => read_auth_summary(codex_home),
            Self::Copilot { .. } => AuthSummary {
                label: "copilot".to_string(),
                quota_compatible: false,
            },
        }
    }

    pub(crate) fn copilot_matches(&self, host: &str, login: &str) -> bool {
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
pub(crate) struct ResponseProfileBinding {
    pub(crate) profile_name: String,
    pub(crate) bound_at: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct AppPaths {
    pub(crate) root: PathBuf,
    pub(crate) state_file: PathBuf,
    pub(crate) managed_profiles_root: PathBuf,
    pub(crate) shared_codex_root: PathBuf,
    pub(crate) legacy_shared_codex_root: PathBuf,
}

impl AppPaths {
    pub(crate) fn discover() -> Result<Self> {
        let root = match env::var_os("PRODEX_HOME") {
            Some(path) => absolutize(PathBuf::from(path))?,
            None => home_dir()
                .context("failed to determine home directory")?
                .join(DEFAULT_PRODEX_DIR),
        };

        Ok(Self {
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: match env::var_os("PRODEX_SHARED_CODEX_HOME") {
                Some(path) => resolve_shared_codex_root(&root, PathBuf::from(path)),
                None => prodex_default_shared_codex_root(&root)?,
            },
            legacy_shared_codex_root: root.join("shared"),
            root,
        })
    }
}

impl AppState {
    pub(crate) fn load_with_recovery(paths: &AppPaths) -> Result<RecoveredLoad<Self>> {
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

    pub(crate) fn load(paths: &AppPaths) -> Result<Self> {
        Ok(Self::load_with_recovery(paths)?.value)
    }

    pub(crate) fn save(&self, paths: &AppPaths) -> Result<()> {
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
