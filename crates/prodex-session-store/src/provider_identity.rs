//! Retire only Prodex's historical transport alias while preserving native provider identity.
use super::repair_transaction::{SessionHeaderRepair, try_native_publication_locks};
use super::session_file::visit_session_lines_limited;
use super::state_db_index::{
    is_codex_state_db_path, path_is_contained_regular_file, persistent_overlay_candidates,
    resolve_state_db_rollout_path,
};
use super::{full_codex_session_id, is_session_metadata_file};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const LEGACY_PROVIDER: &str = "prodex-openai-governed-http";
const MAX_CANDIDATES: usize = 32;
const MAX_SCAN_ENTRIES: usize = 512;
const MAX_DATABASES: usize = 4;
const MAX_HEADER_BYTES: u64 = 1024 * 1024;
const LAUNCH_BUDGET: Duration = Duration::from_secs(2);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ProviderIdentityMigration {
    pub repaired: usize,
    pub deferred: usize,
}

struct Candidate {
    path: PathBuf,
    id: String,
}

struct Header {
    id: String,
    provider: String,
    provider_span: std::ops::Range<usize>,
}

/// Uses indexed reserved aliases first. The bounded active-directory fallback is needed only
/// before native backfill completes; archive discovery stays owned by the native session index.
/// Indexed legacy identities are completed in this launch: a deferred arbitrary batch leaves
/// the native /resume picker selecting unprepared histories. Only unindexed discovery is capped.
pub fn migrate_legacy_openai_provider(
    root: &Path,
    explicit_path: Option<&Path>,
    acquire_maintenance: impl FnOnce() -> Result<Option<File>>,
) -> Result<ProviderIdentityMigration> {
    let started = Instant::now();
    let mut report = ProviderIdentityMigration::default();
    if !fs::symlink_metadata(root).is_ok_and(|metadata| metadata.file_type().is_dir()) {
        return Ok(report);
    }
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(path) = explicit_path {
        add_header_candidate(root, path, &mut candidates, &mut seen);
    }
    let mut databases = Vec::new();
    for entry in fs::read_dir(root)?.take(MAX_SCAN_ENTRIES).flatten() {
        if started.elapsed() >= LAUNCH_BUDGET / 2 {
            break;
        }
        let path = entry.path();
        if is_codex_state_db_path(&path)
            && let Some(version) = path
                .file_stem()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("state_"))
                .and_then(|version| version.parse::<u64>().ok())
        {
            databases.push((version, path));
        }
    }
    databases.sort_by_key(|(version, _)| std::cmp::Reverse(*version));
    databases.truncate(MAX_DATABASES);
    let databases = databases
        .into_iter()
        .map(|(_, path)| path)
        .collect::<Vec<_>>();
    let mut backfill_complete = false;
    for (index, path) in databases.iter().enumerate() {
        let Ok(connection) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        else {
            continue;
        };
        connection.busy_timeout(Duration::ZERO)?;
        // Native maintains this index. Avoid a whole threads-table scan in older/incomplete DBs.
        let Ok(mut query) = connection.prepare(
            "SELECT id, rollout_path FROM threads INDEXED BY idx_threads_provider WHERE model_provider = ?1",
        ) else {
            continue;
        };
        if index == 0 {
            backfill_complete = connection
                .query_row(
                    "SELECT status FROM backfill_state WHERE id = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .is_ok_and(|status| status == "complete");
        }
        let rows = query.query_map(rusqlite::params![LEGACY_PROVIDER], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for (id, rollout_path) in rows.flatten() {
            let Some(id) = full_codex_session_id(id.strip_prefix("thread_").unwrap_or(&id)) else {
                continue;
            };
            let path = resolve_state_db_rollout_path(root, &rollout_path);
            let path = if safe_rollout_path(root, &path) {
                path
            } else {
                let Some(path) = persistent_overlay_candidates(root, &rollout_path)
                    .and_then(|paths| paths.into_iter().find(|path| safe_rollout_path(root, path)))
                else {
                    continue;
                };
                path
            };
            if seen.insert(path.clone()) {
                candidates.push(Candidate {
                    path,
                    id: id.to_string(),
                });
            }
        }
    }
    if !backfill_complete
        && candidates.len() < MAX_CANDIDATES
        && started.elapsed() < LAUNCH_BUDGET / 2
    {
        scan_unindexed_active(root, started, &mut candidates, &mut seen)?;
    }
    if candidates.is_empty() {
        return Ok(report);
    }
    let Some(_maintenance) = acquire_maintenance()? else {
        report.deferred = candidates.len();
        return Ok(report);
    };
    for candidate in candidates {
        match migrate_candidate(root, &candidate, &databases) {
            Ok(true) => report.repaired += 1,
            Ok(false) | Err(_) => report.deferred += 1,
        }
    }
    Ok(report)
}

fn safe_rollout_path(root: &Path, path: &Path) -> bool {
    (path.starts_with(root.join("sessions")) || path.starts_with(root.join("archived_sessions")))
        && is_session_metadata_file(path)
        && path_is_contained_regular_file(root, path)
}

fn add_header_candidate(
    root: &Path,
    path: &Path,
    candidates: &mut Vec<Candidate>,
    seen: &mut BTreeSet<PathBuf>,
) {
    if seen.contains(path) || !safe_rollout_path(root, path) {
        return;
    }
    let mut header = None;
    let mut bytes = 0;
    let result = visit_session_lines_limited(path, MAX_HEADER_BYTES, |line| {
        bytes += line.len() as u64;
        if bytes > MAX_HEADER_BYTES {
            return false;
        }
        if line.trim().is_empty() {
            return true;
        }
        header = parse_header(line, 0).ok();
        false
    });
    if result.is_ok()
        && let Some(header) = header
        && header.provider == LEGACY_PROVIDER
    {
        seen.insert(path.to_path_buf());
        candidates.push(Candidate {
            path: path.to_path_buf(),
            id: header.id,
        });
    }
}

fn scan_unindexed_active(
    root: &Path,
    started: Instant,
    candidates: &mut Vec<Candidate>,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let active = root.join("sessions");
    if !fs::symlink_metadata(&active).is_ok_and(|metadata| metadata.file_type().is_dir()) {
        return Ok(());
    }
    let mut stack = vec![fs::read_dir(active)?];
    let mut entries = 0;
    while let Some(directory) = stack.last_mut() {
        if entries >= MAX_SCAN_ENTRIES
            || candidates.len() >= MAX_CANDIDATES
            || started.elapsed() >= LAUNCH_BUDGET / 2
        {
            break;
        }
        let Some(entry) = directory.next() else {
            stack.pop();
            continue;
        };
        entries += 1;
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            stack.push(fs::read_dir(entry.path())?);
        } else if kind.is_file() && is_session_metadata_file(&entry.path()) {
            add_header_candidate(root, &entry.path(), candidates, seen);
        }
    }
    Ok(())
}

fn parse_header(line: &str, offset: usize) -> Result<Header> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    ensure!(
        value["type"] == "session_meta",
        "not native session metadata"
    );
    let id = value["payload"]["id"]
        .as_str()
        .context("missing session identity")?;
    ensure!(
        full_codex_session_id(id).is_some(),
        "invalid session identity"
    );
    let provider = value["payload"]["model_provider"]
        .as_str()
        .context("missing provider")?;
    let plan = prodex_mojo_core::json::session_report_metadata_json(line)
        .map_err(|error| anyhow::anyhow!("invalid metadata plan: {error:?}"))?;
    let (start, end) = plan.model_provider.context("missing provider span")?;
    let span = line.get(start..end).context("invalid provider span")?;
    ensure!(
        serde_json::from_str::<String>(span)? == provider,
        "provider metadata disagrees"
    );
    Ok(Header {
        id: id.to_string(),
        provider: provider.to_string(),
        provider_span: offset + start..offset + end,
    })
}

fn migrate_candidate(root: &Path, candidate: &Candidate, databases: &[PathBuf]) -> Result<bool> {
    ensure!(
        safe_rollout_path(root, &candidate.path),
        "unsafe session path"
    );
    let Some(_native) = try_native_publication_locks(root, &candidate.id)? else {
        return Ok(false);
    };
    let Some(transaction) =
        SessionHeaderRepair::try_begin(root, &candidate.path, MAX_HEADER_BYTES)?
    else {
        return Ok(false);
    };
    if let Some(repaired) = canonical_provider_header(transaction.prefix(), &candidate.id)? {
        transaction.commit(repaired.as_bytes())?;
    }
    for database in databases {
        ensure!(
            is_codex_state_db_path(database),
            "unsafe state database path"
        );
        let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        connection.busy_timeout(Duration::ZERO)?;
        // Compare the exact indexed path on update, including obsolete overlay paths.
        let Ok(mut query) = connection.prepare(
            "SELECT id, rollout_path FROM threads WHERE (id = ?1 OR id = ?2) AND model_provider = ?3",
        ) else {
            continue;
        };
        let rows = query
            .query_map(
                rusqlite::params![
                    candidate.id,
                    format!("thread_{}", candidate.id),
                    LEGACY_PROVIDER
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(query);
        for (id, expected_path) in rows {
            let indexed = resolve_state_db_rollout_path(root, &expected_path);
            let mapped = persistent_overlay_candidates(root, &expected_path);
            if indexed != candidate.path
                && !mapped.is_some_and(|paths| paths.contains(&candidate.path))
            {
                continue;
            }
            connection.execute(
                "UPDATE threads SET model_provider = 'openai' WHERE id = ?1 AND rollout_path = ?2 AND model_provider = ?3",
                rusqlite::params![id, expected_path, LEGACY_PROVIDER],
            )?;
        }
    }
    Ok(true)
}

fn canonical_provider_header(raw: &str, expected_id: &str) -> Result<Option<String>> {
    let mut offset = 0;
    let line = raw
        .split_inclusive('\n')
        .find(|line| {
            if line.trim().is_empty() {
                offset += line.len();
                false
            } else {
                true
            }
        })
        .context("missing metadata")?;
    let header = parse_header(line, offset)?;
    ensure!(
        header.id == expected_id,
        "indexed session identity disagrees"
    );
    ensure!(
        matches!(header.provider.as_str(), LEGACY_PROVIDER | "openai"),
        "provider changed"
    );
    if header.provider == "openai" {
        return Ok(None);
    }
    let mut repaired = raw.to_string();
    // Native paginated history persists byte offsets. Preserve the prefix width
    // with JSON whitespace so every existing transcript offset remains valid.
    let replacement = format!(
        "{:<width$}",
        "\"openai\"",
        width = header.provider_span.len()
    );
    repaired.replace_range(header.provider_span.clone(), &replacement);
    let end = offset + line.len();
    let replacement = parse_header(&repaired[offset..end], offset)?;
    ensure!(
        replacement.id == header.id && replacement.provider == "openai",
        "ambiguous provider metadata"
    );
    Ok(Some(repaired))
}
