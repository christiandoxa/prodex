use super::*;

pub(super) fn maintain_shared_codex_sessions_after_child_exit() {
    let Ok(paths) = AppPaths::discover() else {
        return;
    };
    let _ = maintain_managed_codex_sessions(&paths);
    let _ =
        prodex_shared_codex_fs::persist_codex_session_image_attachments(&paths.shared_codex_root);
}

pub(super) fn clear_codex_session_binding(session_id: &str) -> Result<()> {
    let paths = AppPaths::discover()?;
    let _lock = acquire_state_file_lock(&paths)?;
    let mut state = AppState::load(&paths)?;
    let compact_key = prodex_runtime_store::runtime_compact_session_lineage_key(session_id);
    let now = chrono::Local::now().timestamp();
    clear_codex_session_continuation_sidecars(
        &paths,
        &state.profiles,
        session_id,
        &compact_key,
        now,
    )?;
    let removed_session = state.session_profile_bindings.remove(session_id).is_some();
    let removed_compact = state
        .session_profile_bindings
        .remove(&compact_key)
        .is_some();
    if removed_session || removed_compact {
        let json =
            serde_json::to_string_pretty(&state).context("failed to serialize prodex state")?;
        write_state_json_atomic(&paths, &json)?;
    }
    Ok(())
}

fn clear_codex_session_continuation_sidecars(
    paths: &AppPaths,
    profiles: &BTreeMap<String, ProfileEntry>,
    session_id: &str,
    compact_key: &str,
    now: i64,
) -> Result<()> {
    let continuations_exist = runtime_continuations_file_path(paths).exists()
        || runtime_continuations_last_good_file_path(paths).exists();
    if continuations_exist {
        let mut continuations = load_runtime_continuations_with_recovery(paths, profiles)?.value;
        clear_codex_session_continuations(&mut continuations, session_id, compact_key, now);
        save_runtime_continuations_for_profiles(paths, &continuations, profiles)?;
    }

    let journal_exists = runtime_continuation_journal_file_path(paths).exists()
        || runtime_continuation_journal_last_good_file_path(paths).exists();
    if journal_exists {
        let mut journal = load_runtime_continuation_journal_with_recovery(paths, profiles)?.value;
        clear_codex_session_continuations(&mut journal.continuations, session_id, compact_key, now);
        save_runtime_continuation_journal_for_profiles(
            paths,
            &journal.continuations,
            profiles,
            now.max(journal.saved_at),
        )?;
    }
    Ok(())
}

fn clear_codex_session_continuations(
    continuations: &mut RuntimeContinuationStore,
    session_id: &str,
    compact_key: &str,
    now: i64,
) {
    continuations.session_profile_bindings.remove(session_id);
    continuations.session_profile_bindings.remove(compact_key);
    continuations.session_id_bindings.remove(session_id);
    continuations.session_id_bindings.remove(compact_key);
    runtime_mark_continuation_status_dead(
        &mut continuations.statuses,
        RuntimeContinuationBindingKind::SessionId,
        session_id,
        now,
    );
    runtime_mark_continuation_status_dead(
        &mut continuations.statuses,
        RuntimeContinuationBindingKind::SessionId,
        compact_key,
        now,
    );
}

pub(super) fn resolve_codex_delete_session_id(codex_args: &[OsString]) -> Result<Option<String>> {
    let Some(selector) = codex_delete_session_selector(codex_args) else {
        return Ok(None);
    };
    if is_full_codex_session_id(selector) {
        return Ok(Some(selector.to_string()));
    }

    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    Ok(
        match prodex_session_store::resolve_session_report_by_id_in_store(
            &paths.shared_codex_root,
            &state,
            selector,
        ) {
            Ok(report) => Some(report.id),
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
    clear_codex_session_binding(session_id)
}
