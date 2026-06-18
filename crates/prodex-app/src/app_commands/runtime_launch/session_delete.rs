use super::*;

pub(super) fn resolve_codex_delete_session_id(codex_args: &[OsString]) -> Result<Option<String>> {
    let Some(selector) = codex_delete_session_selector(codex_args) else {
        return Ok(None);
    };
    if is_full_codex_session_id(selector) {
        return Ok(Some(selector.to_string()));
    }

    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let reports =
        prodex_session_store::collect_session_reports(&paths.shared_codex_root, None, &state)?;
    Ok(
        match prodex_session_store::resolve_session_report_by_id(&reports, selector) {
            Ok(report) => Some(report.id.clone()),
            Err(prodex_session_store::SessionResolveError::Missing { .. })
            | Err(prodex_session_store::SessionResolveError::Ambiguous { .. }) => None,
        },
    )
}

fn codex_delete_session_selector(codex_args: &[OsString]) -> Option<&str> {
    if codex_args.first().and_then(|arg| arg.to_str())? != "delete" {
        return None;
    }
    codex_args
        .iter()
        .skip(1)
        .rev()
        .filter_map(|arg| arg.to_str())
        .find(|arg| {
            let trimmed = arg.trim();
            !trimmed.is_empty() && !trimmed.starts_with('-')
        })
}

fn is_full_codex_session_id(selector: &str) -> bool {
    let bytes = selector.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

pub(super) fn cleanup_codex_deleted_session_binding(session_id: Option<&str>) -> Result<()> {
    let Some(session_id) = session_id else {
        return Ok(());
    };
    let paths = AppPaths::discover()?;
    let _lock = acquire_state_file_lock(&paths)?;
    let mut state = AppState::load(&paths)?;
    let compact_key = prodex_runtime_store::runtime_compact_session_lineage_key(session_id);
    let removed_session = state.session_profile_bindings.remove(session_id).is_some();
    let removed_compact = state
        .session_profile_bindings
        .remove(&compact_key)
        .is_some();
    let removed = removed_session || removed_compact;
    if removed {
        let json =
            serde_json::to_string_pretty(&state).context("failed to serialize prodex state")?;
        write_state_json_atomic(&paths, &json)?;
    }
    Ok(())
}
