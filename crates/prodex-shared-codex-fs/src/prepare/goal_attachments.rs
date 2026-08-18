use super::*;
use crate::image_attachments::rewrite_codex_persisted_attachment_paths;
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::io::Read;

pub(super) fn persist_codex_goal_attachment_paths(codex_home: &Path) -> Result<()> {
    if !codex_home.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(codex_home)
        .with_context(|| format!("failed to read {}", codex_home.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", codex_home.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with("goals_") && file_name.ends_with(".sqlite") {
            persist_codex_goal_attachment_paths_in_db(codex_home, &entry.path())?;
        }
    }
    Ok(())
}

fn persist_codex_goal_attachment_paths_in_db(codex_home: &Path, db_path: &Path) -> Result<()> {
    if !path_looks_like_sqlite_db(db_path) {
        return Ok(());
    }
    let Ok(conn) = Connection::open(db_path) else {
        return Ok(());
    };
    let has_thread_goals = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'thread_goals'",
            [],
            |_| Ok(()),
        )
        .optional()
        .ok()
        .flatten()
        .is_some();
    if !has_thread_goals {
        return Ok(());
    }

    let mut stmt = match conn.prepare("SELECT thread_id, objective FROM thread_goals") {
        Ok(stmt) => stmt,
        Err(_) => return Ok(()),
    };
    let rows = match stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(rows) => rows,
        Err(_) => return Ok(()),
    };
    let mut updates = Vec::new();
    for row in rows {
        let Ok((thread_id, objective)) = row else {
            continue;
        };
        let rewritten = rewrite_codex_persisted_attachment_paths(codex_home, &objective)?;
        if rewritten != objective {
            updates.push((thread_id, rewritten));
        }
    }
    drop(stmt);

    for (thread_id, objective) in updates {
        conn.execute(
            "UPDATE thread_goals SET objective = ? WHERE thread_id = ?",
            params![objective, thread_id],
        )
        .with_context(|| format!("failed to update goal attachments in {}", db_path.display()))?;
    }
    Ok(())
}

pub(super) fn persist_codex_goal_attachment_path_for_thread(
    codex_home: &Path,
    thread_id: &str,
) -> Result<()> {
    for entry in fs::read_dir(codex_home)
        .with_context(|| format!("failed to read {}", codex_home.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", codex_home.display()))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with("goals_") && file_name.ends_with(".sqlite") {
            persist_codex_goal_attachment_path_for_thread_in_db(
                codex_home,
                &entry.path(),
                thread_id,
            )?;
        }
    }
    Ok(())
}

fn persist_codex_goal_attachment_path_for_thread_in_db(
    codex_home: &Path,
    db_path: &Path,
    thread_id: &str,
) -> Result<()> {
    if !path_looks_like_sqlite_db(db_path) {
        return Ok(());
    }
    let conn = Connection::open(db_path)
        .with_context(|| format!("failed to open goal database {}", db_path.display()))?;
    let has_thread_goals = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'thread_goals'",
            [],
            |_| Ok(()),
        )
        .optional()
        .with_context(|| format!("failed to inspect goal database {}", db_path.display()))?
        .is_some();
    if !has_thread_goals {
        return Ok(());
    }
    let Some(objective) = conn
        .query_row(
            "SELECT objective FROM thread_goals WHERE thread_id = ?",
            params![thread_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .with_context(|| {
            format!(
                "failed to read goal {} from {}",
                thread_id,
                db_path.display()
            )
        })?
    else {
        return Ok(());
    };
    let rewritten = rewrite_codex_persisted_attachment_paths(codex_home, &objective)?;
    if rewritten != objective {
        conn.execute(
            "UPDATE thread_goals SET objective = ? WHERE thread_id = ?",
            params![rewritten, thread_id],
        )
        .with_context(|| format!("failed to update goal attachments in {}", db_path.display()))?;
    }
    Ok(())
}

pub(super) fn session_thread_id(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
        prodex_session_store::first_string_value(
            &value,
            &[
                &["payload", "thread_id"],
                &["payload", "threadId"],
                &["thread_id"],
                &["threadId"],
            ],
        )
    })
}

fn path_looks_like_sqlite_db(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() {
        return false;
    }
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut header = [0; 16];
    file.read_exact(&mut header).is_ok() && &header == b"SQLite format 3\0"
}
