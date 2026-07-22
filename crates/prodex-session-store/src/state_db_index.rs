use super::{
    SessionRepairCandidate, SessionRepairMatchKind, codex_session_id_from_path,
    full_codex_session_id, is_session_metadata_file, session_id_matches_selector,
    session_path_id_matches_selector, session_path_id_matching_selector,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn collect_state_db_rollout_paths(
    root: &Path,
    selector: &str,
    paths: &mut Vec<SessionRepairCandidate>,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_codex_state_db_path(&path) {
            continue;
        }
        if let Ok(mut rollout_paths) = state_db_rollout_paths_for_selector(root, &path, selector) {
            paths.append(&mut rollout_paths);
        }
    }
}

pub(super) fn repair_state_db_rollout_path(root: &Path, session_path: &Path) -> Result<()> {
    if !path_is_contained_regular_file(root, session_path) {
        return Ok(());
    }
    let Some(session_id) = codex_session_id_from_path(session_path) else {
        return Ok(());
    };
    let Some(rollout_path) = session_path.to_str() else {
        return Ok(());
    };
    let thread_id = format!("thread_{session_id}");

    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let db_path = entry
            .with_context(|| format!("failed to read entry in {}", root.display()))?
            .path();
        if !is_codex_state_db_path(&db_path) {
            continue;
        }
        let Ok(connection) =
            Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        else {
            continue;
        };
        let Ok(needs_repair) = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM threads WHERE (id = ?1 OR id = ?2) AND rollout_path != ?3)",
                rusqlite::params![session_id, thread_id, rollout_path],
                |row| row.get::<_, bool>(0),
            ) else {
            continue;
        };
        drop(connection);
        if !needs_repair {
            continue;
        }
        let connection = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .with_context(|| {
            format!("failed to open state db {} for repair", db_path.display())
        })?;
        connection
            .execute(
                "UPDATE threads SET rollout_path = ?1 WHERE (id = ?2 OR id = ?3) AND rollout_path != ?1",
                rusqlite::params![rollout_path, session_id, thread_id],
            )
            .with_context(|| format!("failed to repair state db {}", db_path.display()))?;
    }
    Ok(())
}

fn is_codex_state_db_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    file_name.starts_with("state_")
        && file_name.ends_with(".sqlite")
        && path_is_regular_file_no_symlink(path)
}

fn state_db_rollout_paths_for_selector(
    codex_home: &Path,
    db_path: &Path,
    selector: &str,
) -> Result<Vec<SessionRepairCandidate>> {
    let connection = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("failed to open state db {}", db_path.display()))?;
    let mut paths = Vec::new();
    if let Some(full_selector) = full_codex_session_id(selector) {
        let mut statement = connection
            .prepare("SELECT id, rollout_path FROM threads WHERE id = ?1 OR id = ?2")
            .with_context(|| format!("failed to query state db {}", db_path.display()))?;
        let thread_selector = format!("thread_{full_selector}");
        let rows = statement
            .query_map([full_selector, thread_selector.as_str()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .with_context(|| format!("failed to read state db {}", db_path.display()))?;
        append_state_db_rollout_rows(codex_home, db_path, selector, rows, &mut paths)?;
    } else {
        let mut statement = connection
            .prepare("SELECT id, rollout_path FROM threads")
            .with_context(|| format!("failed to query state db {}", db_path.display()))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .with_context(|| format!("failed to read state db {}", db_path.display()))?;
        append_state_db_rollout_rows(codex_home, db_path, selector, rows, &mut paths)?;
    }
    Ok(paths)
}

fn append_state_db_rollout_rows(
    codex_home: &Path,
    db_path: &Path,
    selector: &str,
    rows: impl IntoIterator<Item = rusqlite::Result<(String, String)>>,
    paths: &mut Vec<SessionRepairCandidate>,
) -> Result<()> {
    for row in rows {
        let (thread_id, rollout_path) =
            row.with_context(|| format!("failed to read state db {}", db_path.display()))?;
        let Some(match_kind) = state_db_rollout_row_match_kind(&thread_id, &rollout_path, selector)
        else {
            continue;
        };
        let resolved_session_id =
            state_db_rollout_row_session_id(&thread_id, &rollout_path, selector);
        let path = resolve_state_db_rollout_path(codex_home, &rollout_path);
        if path_is_contained_regular_file(codex_home, &path) && is_session_metadata_file(&path) {
            paths.push(SessionRepairCandidate {
                path,
                state_db_match_kind: Some(match_kind),
                resolved_session_id,
            });
        }
    }
    Ok(())
}

fn state_db_rollout_row_match_kind(
    thread_id: &str,
    rollout_path: &str,
    selector: &str,
) -> Option<SessionRepairMatchKind> {
    let normalized_thread_id = thread_id.strip_prefix("thread_").unwrap_or(thread_id);
    if session_id_matches_selector(thread_id, selector, true)
        || session_id_matches_selector(normalized_thread_id, selector, true)
        || session_path_id_matches_selector(Path::new(rollout_path), selector, true)
    {
        return Some(SessionRepairMatchKind::Exact);
    }
    if session_id_matches_selector(thread_id, selector, false)
        || session_id_matches_selector(normalized_thread_id, selector, false)
        || session_path_id_matches_selector(Path::new(rollout_path), selector, false)
    {
        return Some(SessionRepairMatchKind::Prefix);
    }
    None
}

fn state_db_rollout_row_session_id(
    thread_id: &str,
    rollout_path: &str,
    selector: &str,
) -> Option<String> {
    let normalized_thread_id = thread_id.strip_prefix("thread_").unwrap_or(thread_id);
    if session_id_matches_selector(normalized_thread_id, selector, false)
        && full_codex_session_id(normalized_thread_id).is_some()
    {
        return Some(normalized_thread_id.to_string());
    }
    if session_id_matches_selector(thread_id, selector, false)
        && full_codex_session_id(thread_id).is_some()
    {
        return Some(thread_id.to_string());
    }
    session_path_id_matching_selector(Path::new(rollout_path), selector, false)
}

fn resolve_state_db_rollout_path(codex_home: &Path, rollout_path: &str) -> PathBuf {
    let path = PathBuf::from(rollout_path);
    if path.is_absolute() {
        path
    } else {
        codex_home.join(path)
    }
}

fn path_is_regular_file_no_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_file())
}

fn path_is_contained_regular_file(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut current = root.to_path_buf();
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(component) = component else {
            return false;
        };
        current.push(component);
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            return false;
        };
        if metadata.file_type().is_symlink() || (components.peek().is_some() && !metadata.is_dir())
        {
            return false;
        }
    }
    path_is_regular_file_no_symlink(&current)
}
