use super::*;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionReport {
    pub(crate) id: String,
    pub(crate) thread_name: Option<String>,
    pub(crate) updated_at: Option<String>,
    pub(crate) cwd: Option<String>,
    pub(crate) profile: Option<String>,
    pub(crate) path: String,
    #[serde(skip)]
    updated_sort_key: i64,
    #[serde(skip)]
    cwd_path: Option<PathBuf>,
}

pub(crate) fn handle_session(command: SessionCommands) -> Result<()> {
    match command {
        SessionCommands::List(args) => {
            let reports = load_session_reports(None, args.limit)?;
            print_session_reports(&reports, args.json, "No sessions found")
        }
        SessionCommands::Current(args) => {
            let cwd =
                args.cwd.map(absolutize).transpose()?.unwrap_or(
                    env::current_dir().context("failed to determine current directory")?,
                );
            let reports = load_session_reports(Some(&cwd), args.limit)?;
            print_session_reports(
                &reports,
                args.json,
                &format!("No sessions found for {}", cwd.display()),
            )
        }
    }
}

fn load_session_reports(
    current_dir: Option<&Path>,
    limit: Option<usize>,
) -> Result<Vec<SessionReport>> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let mut reports = collect_session_reports(&paths.shared_codex_root, current_dir, &state)?;
    if let Some(limit) = limit {
        reports.truncate(limit);
    }
    Ok(reports)
}

fn print_session_reports(reports: &[SessionReport], json: bool, empty_message: &str) -> Result<()> {
    if json {
        let json =
            serde_json::to_string_pretty(reports).context("failed to render session JSON")?;
        print_stdout_line(&json);
        return Ok(());
    }

    if reports.is_empty() {
        print_panel(
            "Sessions",
            &[("Status".to_string(), empty_message.to_string())],
        );
        return Ok(());
    }

    print_stdout_text(&render_session_reports(reports));
    Ok(())
}

pub(crate) fn collect_session_reports(
    shared_codex_root: &Path,
    current_dir: Option<&Path>,
    state: &AppState,
) -> Result<Vec<SessionReport>> {
    let sessions_root = shared_codex_root.join("sessions");
    let mut session_paths = Vec::new();
    collect_session_paths(&sessions_root, &mut session_paths)?;
    session_paths.sort();

    let current_dir = current_dir.map(normalize_path_for_compare);
    let mut reports = Vec::new();
    for path in session_paths {
        let report = read_session_report(&path, state)?;
        if current_dir.as_ref().is_some_and(|current_dir| {
            report
                .cwd_path
                .as_ref()
                .is_none_or(|cwd| normalize_path_for_compare(cwd) != *current_dir)
        }) {
            continue;
        }
        reports.push(report);
    }

    reports.sort_by(|left, right| {
        right
            .updated_sort_key
            .cmp(&left.updated_sort_key)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(reports)
}

fn collect_session_paths(root: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry.with_context(|| format!("failed to read entry in {}", root.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if file_type.is_dir() {
            collect_session_paths(&path, paths)?;
        } else if file_type.is_file() && is_session_metadata_file(&path) {
            paths.push(path);
        }
    }

    Ok(())
}

fn is_session_metadata_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "jsonl" | "json"))
}

fn read_session_report(path: &Path, state: &AppState) -> Result<SessionReport> {
    let mut report = SessionReport {
        id: session_id_from_path(path),
        thread_name: None,
        updated_at: None,
        cwd: None,
        profile: None,
        path: path.display().to_string(),
        updated_sort_key: file_modified_epoch(path).unwrap_or(0),
        cwd_path: None,
    };
    report.updated_at = Some(format_epoch(report.updated_sort_key));

    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            apply_session_value(&mut report, &value);
        } else {
            apply_session_json_lines(&mut report, raw.lines());
        }
    } else {
        let file = fs::File::open(path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        let reader = io::BufReader::new(file);
        for line in reader.lines() {
            let line =
                line.with_context(|| format!("failed to read line from {}", path.display()))?;
            apply_session_json_line(&mut report, &line);
        }
    }

    if let Some(binding) = state.session_profile_bindings.get(&report.id) {
        report.profile = Some(binding.profile_name.clone());
    }
    Ok(report)
}

fn apply_session_json_lines<'a>(
    report: &mut SessionReport,
    lines: impl IntoIterator<Item = &'a str>,
) {
    for line in lines {
        apply_session_json_line(report, line);
    }
}

fn apply_session_json_line(report: &mut SessionReport, line: &str) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        apply_session_value(report, &value);
    }
}

fn apply_session_value(report: &mut SessionReport, value: &serde_json::Value) {
    if let Some(id) = first_string_value(
        value,
        &[
            &["payload", "id"],
            &["payload", "session_id"],
            &["id"],
            &["session_id"],
        ],
    ) {
        report.id = id;
    }

    if let Some(thread_name) = first_string_value(
        value,
        &[
            &["payload", "thread_name"],
            &["payload", "title"],
            &["payload", "metadata", "thread_name"],
            &["thread_name"],
            &["title"],
            &["metadata", "thread_name"],
        ],
    ) {
        report.thread_name = Some(thread_name);
    }

    if let Some(cwd) = first_string_value(
        value,
        &[
            &["payload", "cwd"],
            &["payload", "metadata", "cwd"],
            &["payload", "workdir"],
            &["cwd"],
            &["metadata", "cwd"],
            &["workdir"],
        ],
    ) {
        report.cwd_path = Some(PathBuf::from(&cwd));
        report.cwd = Some(cwd);
    }

    if let Some(updated_at) = first_string_value(
        value,
        &[
            &["updated_at"],
            &["timestamp"],
            &["payload", "updated_at"],
            &["payload", "timestamp"],
        ],
    ) {
        report.updated_sort_key =
            timestamp_label_sort_key(&updated_at).unwrap_or(report.updated_sort_key);
        report.updated_at = Some(updated_at);
    } else if let Some(epoch) = first_i64_value(
        value,
        &[
            &["updated_at"],
            &["ts"],
            &["timestamp"],
            &["payload", "updated_at"],
            &["payload", "ts"],
            &["payload", "timestamp"],
        ],
    ) {
        report.updated_sort_key = epoch;
        report.updated_at = Some(format_epoch(epoch));
    }
}

fn first_string_value(value: &serde_json::Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| value_at_path(value, path).and_then(serde_json::Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn first_i64_value(value: &serde_json::Value, paths: &[&[&str]]) -> Option<i64> {
    paths
        .iter()
        .find_map(|path| value_at_path(value, path).and_then(serde_json::Value::as_i64))
}

fn value_at_path<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn session_id_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-session")
        .to_string()
}

fn file_modified_epoch(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_epoch)
}

fn system_time_epoch(time: SystemTime) -> Option<i64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
}

fn timestamp_label_sort_key(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.timestamp())
        .ok()
        .or_else(|| value.parse::<i64>().ok())
}

fn format_epoch(epoch: i64) -> String {
    Local
        .timestamp_opt(epoch, 0)
        .single()
        .map(|timestamp| timestamp.format("%Y-%m-%d %H:%M:%S %Z").to_string())
        .unwrap_or_else(|| epoch.to_string())
}

fn render_session_reports(reports: &[SessionReport]) -> String {
    let total_width = current_cli_width();
    let widths = session_report_column_widths(total_width);
    let mut lines = vec![section_header_with_width("Sessions", total_width)];
    lines.push(format_session_report_row(
        "ID", "UPDATED", "THREAD", "CWD", "PATH", widths,
    ));
    lines.push("-".repeat(text_width(lines.last().map(String::as_str).unwrap_or(""))));

    for report in reports {
        lines.push(format_session_report_row(
            &report.id,
            report.updated_at.as_deref().unwrap_or("-"),
            report.thread_name.as_deref().unwrap_or("-"),
            report.cwd.as_deref().unwrap_or("-"),
            &report.path,
            widths,
        ));
        if let Some(profile) = report.profile.as_deref() {
            lines.push(format!("  profile: {profile}"));
        }
    }

    lines.join("\n")
}

#[derive(Clone, Copy)]
struct SessionReportColumnWidths {
    id: usize,
    updated: usize,
    thread: usize,
    cwd: usize,
    path: usize,
}

fn session_report_column_widths(total_width: usize) -> SessionReportColumnWidths {
    let gap_width = text_width(CLI_TABLE_GAP) * 4;
    let available = total_width.saturating_sub(gap_width).max(60);
    let id = (available / 5).clamp(12, 26);
    let updated = (available / 5).clamp(12, 22);
    let thread = (available / 5).clamp(12, 24);
    let remaining = available.saturating_sub(id + updated + thread);
    let cwd = (remaining / 2).max(12);
    let path = remaining.saturating_sub(cwd).max(12);
    SessionReportColumnWidths {
        id,
        updated,
        thread,
        cwd,
        path,
    }
}

fn format_session_report_row(
    id: &str,
    updated: &str,
    thread_name: &str,
    cwd: &str,
    path: &str,
    widths: SessionReportColumnWidths,
) -> String {
    format!(
        "{:<id_w$}{gap}{:<updated_w$}{gap}{:<thread_w$}{gap}{:<cwd_w$}{gap}{:<path_w$}",
        fit_cell(id, widths.id),
        fit_cell(updated, widths.updated),
        fit_cell(thread_name, widths.thread),
        fit_cell(cwd, widths.cwd),
        fit_cell(path, widths.path),
        gap = CLI_TABLE_GAP,
        id_w = widths.id,
        updated_w = widths.updated,
        thread_w = widths.thread,
        cwd_w = widths.cwd,
        path_w = widths.path,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_reports_parse_codex_jsonl_metadata() {
        let root = test_temp_dir("session-jsonl");
        let sessions = root.join("sessions/2026/04/29");
        fs::create_dir_all(&sessions).expect("session dir should be created");
        let cwd = root.join("workspace");
        fs::create_dir_all(&cwd).expect("workspace should be created");
        fs::write(
            sessions.join("session-a.jsonl"),
            format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"sess-a\",\"thread_name\":\"Issue triage\",\"cwd\":\"{}\"}}}}\n{{\"timestamp\":\"2026-04-29T12:30:00Z\",\"type\":\"event\"}}\n",
                cwd.display()
            ),
        )
        .expect("session should be written");

        let reports = collect_session_reports(&root, None, &AppState::default())
            .expect("sessions should collect");

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].id, "sess-a");
        assert_eq!(reports[0].thread_name.as_deref(), Some("Issue triage"));
        assert_eq!(
            reports[0].cwd.as_deref(),
            Some(cwd.to_string_lossy().as_ref())
        );
        assert_eq!(
            reports[0].updated_at.as_deref(),
            Some("2026-04-29T12:30:00Z")
        );
    }

    #[test]
    fn session_current_filters_by_cwd() {
        let root = test_temp_dir("session-current");
        let sessions = root.join("sessions");
        let current = root.join("current");
        let other = root.join("other");
        fs::create_dir_all(&sessions).expect("session dir should be created");
        fs::create_dir_all(&current).expect("current dir should be created");
        fs::create_dir_all(&other).expect("other dir should be created");
        fs::write(
            sessions.join("current.jsonl"),
            format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"current\",\"cwd\":\"{}\"}}}}\n",
                current.display()
            ),
        )
        .expect("current session should be written");
        fs::write(
            sessions.join("other.jsonl"),
            format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"other\",\"cwd\":\"{}\"}}}}\n",
                other.display()
            ),
        )
        .expect("other session should be written");

        let reports = collect_session_reports(&root, Some(&current), &AppState::default())
            .expect("sessions should collect");

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].id, "current");
    }

    #[test]
    fn quota_auth_filter_matches_labels_and_compatibility() {
        let no_auth = AuthSummary {
            label: "no-auth".to_string(),
            quota_compatible: false,
        };
        let chatgpt = AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        };

        assert!(QuotaAuthFilter::parse("no-auth").unwrap().matches(&no_auth));
        assert!(!QuotaAuthFilter::parse("no-auth").unwrap().matches(&chatgpt));
        assert!(
            QuotaAuthFilter::parse("quota-compatible")
                .unwrap()
                .matches(&chatgpt)
        );
        assert!(
            QuotaAuthFilter::parse("non-quota-compatible")
                .unwrap()
                .matches(&no_auth)
        );
    }

    fn test_temp_dir(name: &str) -> PathBuf {
        let root = env::temp_dir().join(format!(
            "prodex-{name}-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("old temp dir should be removed");
        }
        fs::create_dir_all(&root).expect("temp dir should be created");
        root
    }
}
