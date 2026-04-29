use super::*;

pub(crate) fn audit_log_event_best_effort(
    component: &str,
    action: &str,
    outcome: &str,
    details: serde_json::Value,
) {
    let _ = append_audit_event(component, action, outcome, details);
}

pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path_for_compare(left) == normalize_path_for_compare(right)
}

pub(crate) fn normalize_path_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(crate) fn absolutize(path: PathBuf) -> Result<PathBuf> {
    prodex_core::absolutize(path)
}

pub(crate) fn legacy_default_codex_home() -> Result<PathBuf> {
    prodex_core::legacy_default_codex_home()
}

pub(crate) fn prodex_previous_default_shared_codex_root(root: &Path) -> PathBuf {
    prodex_core::prodex_previous_default_shared_codex_root(root)
}

pub(crate) fn select_default_codex_home(
    shared_codex_root: &Path,
    legacy_codex_home: &Path,
    override_active: bool,
) -> PathBuf {
    if override_active || shared_codex_root.exists() || !legacy_codex_home.exists() {
        shared_codex_root.to_path_buf()
    } else {
        legacy_codex_home.to_path_buf()
    }
}

pub(crate) fn default_codex_home(paths: &AppPaths) -> Result<PathBuf> {
    let legacy = legacy_default_codex_home()?;
    Ok(select_default_codex_home(
        &paths.shared_codex_root,
        &legacy,
        env::var_os("PRODEX_SHARED_CODEX_HOME").is_some(),
    ))
}
