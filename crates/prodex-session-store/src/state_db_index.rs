use super::{
    SessionRepairCandidate, SessionRepairMatchKind, codex_session_id_from_path,
    full_codex_session_id, is_session_metadata_file, session_file_has_resume_metadata_for_selector,
    session_id_matches_selector, session_path_id_matches_selector,
    session_path_id_matching_selector,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::path::{Component, Path, PathBuf};

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

/// Repoints stale overlay state rows to a verified persistent rollout path.
///
/// Codex does not expose this repair through its app-server protocol. Keep the update narrow:
/// only an existing row for the rollout's UUID whose current value is an overlay path is changed,
/// and the write uses that value as an expected-old-path compare-and-swap.
pub fn repair_state_db_rollout_path(root: &Path, session_path: &Path) -> Result<bool> {
    if !path_is_contained_regular_file(root, session_path) {
        return Ok(false);
    }
    let Some(session_id) = codex_session_id_from_path(session_path) else {
        return Ok(false);
    };
    let Some(rollout_path) = session_path.to_str() else {
        return Ok(false);
    };
    let mut repaired = false;

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
        let Ok(mut statement) = connection.prepare(
            "SELECT id, rollout_path FROM threads WHERE (id = ?1 OR id = ?2) AND rollout_path != ?3 AND instr(rollout_path, '.prodex-overlay-') > 0",
        ) else {
            continue;
        };
        let Ok(rows) = statement.query_map(
            rusqlite::params![session_id, format!("thread_{session_id}"), rollout_path],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ) else {
            continue;
        };
        let candidates = rows.flatten().collect::<Vec<_>>();
        drop(statement);
        drop(connection);
        for (thread_id, expected_path) in candidates {
            repaired |= repair_state_db_rollout_path_if_current(
                &db_path,
                &session_id,
                &thread_id,
                &expected_path,
                rollout_path,
            )?;
        }
    }
    Ok(repaired)
}

/// Repairs stale overlay rollout paths without scanning the rollout history.
///
/// This is intentionally limited to state rows containing a Prodex overlay marker. For each
/// row, the same relative path is checked in the persistent active and archived roots and its
/// session metadata must identify the indexed thread before SQLite is changed.
pub fn repair_stale_overlay_rollout_paths(root: &Path) -> Result<usize> {
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(0);
    };
    let mut repaired = 0;
    for entry in entries.flatten() {
        let db_path = entry.path();
        if !is_codex_state_db_path(&db_path) {
            continue;
        }
        let Ok(connection) =
            Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        else {
            continue;
        };
        let Ok(mut statement) = connection.prepare(
            "SELECT id, rollout_path FROM threads WHERE instr(rollout_path, '.prodex-overlay-') > 0",
        ) else {
            continue;
        };
        let Ok(rows) = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) else {
            continue;
        };
        let candidates = rows.flatten().collect::<Vec<_>>();
        drop(statement);
        drop(connection);
        for (thread_id, stale_path) in candidates {
            let Some(session_id) = state_row_session_id(&thread_id, &stale_path) else {
                continue;
            };
            let Some(persistent_path) =
                persistent_overlay_replacement(root, &stale_path, &session_id)
            else {
                continue;
            };
            let Some(persistent_path) = persistent_path.to_str() else {
                continue;
            };
            if repair_state_db_rollout_path_if_current(
                &db_path,
                &session_id,
                &thread_id,
                &stale_path,
                persistent_path,
            )? {
                repaired += 1;
            }
        }
    }
    Ok(repaired)
}

/// Repairs stale overlay rows for one known thread UUID.
///
/// The normal picker repair deliberately uses only the overlay's relative path so it stays
/// bounded. A direct resume knows the UUID, so it may perform the narrowly scoped persistent
/// lookup required when promotion changed the rollout's date directory or filename.
pub fn repair_stale_overlay_rollout_path_for_session(
    root: &Path,
    session_id: &str,
) -> Result<usize> {
    let Some(session_id) = full_codex_session_id(session_id).map(str::to_owned) else {
        return Ok(0);
    };
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(0);
    };
    let mut repaired = 0;
    for entry in entries.flatten() {
        let db_path = entry.path();
        if !is_codex_state_db_path(&db_path) {
            continue;
        }
        let Ok(connection) =
            Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        else {
            continue;
        };
        let Ok(mut statement) = connection.prepare(
            "SELECT id, rollout_path FROM threads WHERE (id = ?1 OR id = ?2) AND instr(rollout_path, '.prodex-overlay-') > 0",
        ) else {
            continue;
        };
        let Ok(rows) = statement.query_map(
            rusqlite::params![session_id, format!("thread_{session_id}")],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ) else {
            continue;
        };
        let candidates = rows.flatten().collect::<Vec<_>>();
        drop(statement);
        drop(connection);
        for (thread_id, stale_path) in candidates {
            let Some(persistent_path) =
                persistent_overlay_replacement(root, &stale_path, &session_id)
                    .or_else(|| find_persistent_rollout_by_session_id(root, &session_id))
            else {
                continue;
            };
            let Some(persistent_path) = persistent_path.to_str() else {
                continue;
            };
            if repair_state_db_rollout_path_if_current(
                &db_path,
                &session_id,
                &thread_id,
                &stale_path,
                persistent_path,
            )? {
                repaired += 1;
            }
        }
    }
    Ok(repaired)
}

fn repair_state_db_rollout_path_if_current(
    db_path: &Path,
    session_id: &str,
    thread_id: &str,
    expected_path: &str,
    persistent_path: &str,
) -> Result<bool> {
    let connection = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .with_context(|| format!("failed to open state db {} for repair", db_path.display()))?;
    let changed = connection
        .execute(
            "UPDATE threads SET rollout_path = ?1 WHERE (id = ?2 OR id = ?3 OR id = ?4) AND rollout_path = ?5",
            rusqlite::params![persistent_path, session_id, format!("thread_{session_id}"), thread_id, expected_path],
        )
        .with_context(|| format!("failed to repair state db {}", db_path.display()))?;
    Ok(changed > 0)
}

fn state_row_session_id(thread_id: &str, rollout_path: &str) -> Option<String> {
    let candidate = thread_id.strip_prefix("thread_").unwrap_or(thread_id);
    full_codex_session_id(candidate)
        .map(str::to_owned)
        .or_else(|| codex_session_id_from_path(Path::new(rollout_path)))
}

fn persistent_overlay_replacement(
    root: &Path,
    stale_path: &str,
    session_id: &str,
) -> Option<PathBuf> {
    let path = Path::new(stale_path);
    let components = path.components().collect::<Vec<_>>();
    let overlay_index = components.iter().position(|component| {
        matches!(component, Component::Normal(name) if name.to_string_lossy().starts_with(".prodex-overlay-"))
    })?;
    let storage_index = components
        .iter()
        .enumerate()
        .skip(overlay_index + 1)
        .find_map(|(index, component)| {
            matches!(component, Component::Normal(name) if name.to_string_lossy() == "sessions" || name.to_string_lossy() == "archived_sessions")
                .then_some(index)
        })?;
    let storage = match components[storage_index] {
        Component::Normal(name) => name.to_string_lossy(),
        _ => return None,
    };
    let relative = components.iter().skip(storage_index + 1).try_fold(
        PathBuf::new(),
        |mut relative, component| {
            let Component::Normal(component) = component else {
                return None;
            };
            relative.push(component);
            Some(relative)
        },
    )?;
    if relative.as_os_str().is_empty() {
        return None;
    }

    let alternate_storage = if storage == "sessions" {
        "archived_sessions"
    } else {
        "sessions"
    };
    [storage.as_ref(), alternate_storage]
        .into_iter()
        .map(|storage| root.join(storage).join(&relative))
        .find(|candidate| {
            path_is_contained_regular_file(root, candidate)
                && is_session_metadata_file(candidate)
                && session_file_has_resume_metadata_for_selector(candidate, session_id)
                    .unwrap_or(false)
        })
}

fn find_persistent_rollout_by_session_id(root: &Path, session_id: &str) -> Option<PathBuf> {
    let mut pending = ["sessions", "archived_sessions"]
        .into_iter()
        .map(|directory| root.join(directory))
        .collect::<Vec<_>>();
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if let Some(path) = persistent_rollout_candidate(root, path, session_id) {
                return Some(path);
            }
        }
    }
    None
}

fn persistent_rollout_candidate(root: &Path, path: PathBuf, session_id: &str) -> Option<PathBuf> {
    if !is_session_metadata_file(&path) || !path_is_contained_regular_file(root, &path) {
        return None;
    }
    if codex_session_id_from_path(&path).as_deref() == Some(session_id)
        || session_file_has_resume_metadata_for_selector(&path, session_id).unwrap_or(false)
    {
        Some(path)
    } else {
        None
    }
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
