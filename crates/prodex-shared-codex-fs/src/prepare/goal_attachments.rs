use super::*;
use crate::image_attachments::{
    persist_codex_goal_objective_attachments, rewrite_codex_persisted_attachment_paths,
};
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
    if !path_looks_like_sqlite_db(db_path)? {
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
        persist_codex_goal_objective_attachments(codex_home, &rewritten)?;
        if rewritten != objective {
            updates.push((thread_id, objective, rewritten));
        }
    }
    drop(stmt);

    for (thread_id, previous, rewritten) in updates {
        update_goal_attachment_reference(&conn, db_path, &thread_id, &previous, &rewritten)?;
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
    if !path_looks_like_sqlite_db(db_path)? {
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
    persist_codex_goal_objective_attachments(codex_home, &rewritten)?;
    if rewritten != objective {
        update_goal_attachment_reference(&conn, db_path, thread_id, &objective, &rewritten)?;
    }
    Ok(())
}

// Repair the independent SQLite row without overwriting a newer objective,
// resuming its status, resetting counters or touching another thread's goal.
fn update_goal_attachment_reference(
    conn: &Connection,
    db_path: &Path,
    thread_id: &str,
    previous: &str,
    rewritten: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE thread_goals SET objective = ?1 WHERE thread_id = ?2 AND objective = ?3",
        params![rewritten, thread_id, previous],
    )
    .with_context(|| format!("failed to update goal attachments in {}", db_path.display()))?;
    Ok(())
}

/// Reading the immutable header does not require stopping an unrelated child.
/// Never rewrite rollouts without the existing exclusive maintenance lock.
pub(super) fn persist_goal_attachments_for_session_file(
    codex_home: &Path,
    session_file: &Path,
) -> Result<()> {
    if !prodex_core::path_is_under_root(codex_home, session_file) {
        return Ok(());
    }
    let file = prodex_core::open_regular_file_no_follow(session_file)?;
    let mut reader: Box<dyn Read> = if session_file.to_string_lossy().ends_with(".jsonl.zst") {
        Box::new(zstd::stream::read::Decoder::new(file)?)
    } else {
        Box::new(file)
    };
    let mut prefix = Vec::new();
    reader.by_ref().take(64 * 1024).read_to_end(&mut prefix)?;
    if let Some(id) = session_thread_id(&String::from_utf8_lossy(&prefix)) {
        persist_codex_goal_attachment_path_for_thread(codex_home, &id)?;
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
        .or_else(|| {
            // Native Codex rollouts use session_meta.payload.id, not thread_id.
            // This is a wire-envelope projection; arbitrary message/tool IDs
            // must never select another thread's persisted goal.
            (value.get("type").and_then(serde_json::Value::as_str) == Some("session_meta"))
                .then(|| prodex_session_store::first_string_value(&value, &[&["payload", "id"]]))
                .flatten()
        })
    })
}

fn path_looks_like_sqlite_db(path: &Path) -> Result<bool> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(false);
    };
    if !metadata.file_type().is_file() {
        return Ok(false);
    }
    let Ok(mut file) = fs::File::open(path) else {
        return Ok(false);
    };
    let mut header = [0; 16];
    if file.read_exact(&mut header).is_err() {
        return Ok(false);
    }
    prodex_mojo_core::shared_attachment_policy::sqlite_header_is_sqlite(&header)
        .map_err(|error| anyhow::anyhow!("Mojo SQLite header policy failed: {error:?}"))
}

#[cfg(test)]
#[path = "../../tests/src/reconnect_attachments.rs"]
mod reconnect_tests;

#[cfg(test)]
#[path = "../../tests/src/resume_media.rs"]
mod resume_media_tests;
