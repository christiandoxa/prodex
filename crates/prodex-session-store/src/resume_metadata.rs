//! Startup metadata must not deserialize an entire multi-GiB conversation.
//! Native Codex remains responsible for replaying history and restoring omitted turn settings.
use super::*;
use std::io::{Read, Seek, SeekFrom};

const HEADER_BYTES: u64 = 1024 * 1024;
const TAIL_BYTES: u64 = if cfg!(test) {
    64 * 1024
} else {
    8 * 1024 * 1024
};

/// Resolve an exact or unambiguous abbreviated launch target using metadata only.
/// Plain rollouts read their header and a bounded complete-record tail; settings absent
/// from that tail are left to native Codex rather than recovered by scanning all history.
pub fn resolve_resume_launch_report(
    root: &Path,
    state: &AppState,
    selector: &str,
) -> std::result::Result<SessionReport, SessionResolveError> {
    let selector = selector.trim();
    let missing = || SessionResolveError::Missing {
        selector: selector.into(),
    };
    let mut paths = Vec::new();
    if full_codex_session_id(selector).is_some() {
        collect_exact_session_paths_for_selector(root, selector, &mut paths)
            .map_err(|_| missing())?;
    }
    // Older stores can use generic filenames without a native index. Their headers,
    // not their transcript bodies, remain the authority for UUID resolution.
    if paths.is_empty() {
        collect_session_paths(&root.join(SESSIONS_DIR), &mut paths).map_err(|_| missing())?;
        collect_session_paths(&root.join(ARCHIVED_SESSIONS_DIR), &mut paths)
            .map_err(|_| missing())?;
    }
    paths.sort();
    paths.dedup();
    let reports = paths
        .into_iter()
        .filter_map(|path| read_launch_report(&path, state).ok().flatten())
        .collect::<Vec<_>>();
    resolve_session_report_by_id(&reports, selector).cloned()
}

/// Collect metadata for native resume-last selection without replaying tool/message bodies.
pub fn collect_resume_launch_reports(
    root: &Path,
    filter: SessionReportFilter<'_>,
    state: &AppState,
) -> Result<Vec<SessionReport>> {
    let mut paths = Vec::new();
    collect_session_paths(&root.join(SESSIONS_DIR), &mut paths)?;
    collect_session_paths(&root.join(ARCHIVED_SESSIONS_DIR), &mut paths)?;
    let mut reports = Vec::new();
    for path in paths {
        if let Some(report) = read_launch_report(&path, state)?
            && session_report_matches_filter(&report, &filter)
        {
            reports.push(report);
        }
    }
    sort_session_reports(&mut reports);
    Ok(reports)
}

pub(super) fn read_launch_report(path: &Path, state: &AppState) -> Result<Option<SessionReport>> {
    if path.extension().and_then(|value| value.to_str()) == Some("json") {
        return read_session_report(path, state);
    }
    // Hold one validated descriptor for prefix and tail: an atomic replacement
    // must not combine the header of one generation with another generation's settings.
    let file = session_file::open_session_regular_file(path)?;
    let mut report = SessionReport::from_path(path, file_modified_epoch(path).unwrap_or(0));
    let mut header_bytes = 0;
    let mut valid = false;
    session_file::visit_open_session_lines_limited(
        path,
        file.try_clone()?,
        HEADER_BYTES,
        |line| {
            header_bytes += line.len() as u64;
            if header_bytes > HEADER_BYTES {
                return false;
            }
            if line.trim().is_empty() {
                return true;
            }
            valid = session_line_starts_resume_metadata(line);
            if valid {
                apply_session_json_line(&mut report, line);
            }
            false
        },
    )?;
    if !valid {
        return Ok(None);
    }
    if !session_file::is_compressed_session_file(path) {
        read_complete_tail(file, header_bytes, &mut report)?;
    }
    if let Some(binding) = state.session_profile_bindings.get(&report.id) {
        report.set_profile(Some(binding.profile_name.clone()));
    }
    Ok(Some(report))
}

fn read_complete_tail(
    mut file: fs::File,
    header_bytes: u64,
    report: &mut SessionReport,
) -> Result<()> {
    let len = file.metadata()?.len();
    let start = len.saturating_sub(TAIL_BYTES).max(header_bytes);
    if start >= len {
        return Ok(());
    }
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(len - start).read_to_end(&mut bytes)?;
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n');
    if start > header_bytes {
        // The seek may bisect UTF-8, a large tool result, or an encoded attachment.
        // Never interpret that fragment as a new JSON record.
        lines.next();
    }
    for line in lines {
        if line.len() as u64 > HEADER_BYTES {
            continue;
        }
        if let Ok(line) = std::str::from_utf8(line) {
            apply_session_json_line(report, line);
        }
    }
    Ok(())
}
