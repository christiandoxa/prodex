use super::*;
pub(crate) use prodex_app_reports::SessionReport;
use prodex_app_reports::{
    apply_session_json_line, apply_session_json_lines, apply_session_value,
    is_session_metadata_file, sort_session_reports,
};

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

    let display_reports = session_report_display_rows(reports);
    print_stdout_text(&render_session_reports(&display_reports));
    Ok(())
}

fn session_report_display_rows(reports: &[SessionReport]) -> Vec<SessionReportDisplay<'_>> {
    reports
        .iter()
        .map(|report| SessionReportDisplay {
            id: &report.id,
            updated_at: report.updated_at.as_deref(),
            thread_name: report.thread_name.as_deref(),
            cwd: report.cwd.as_deref(),
            profile: report.profile.as_deref(),
            path: &report.path,
        })
        .collect()
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

    let mut reports = Vec::new();
    for path in session_paths {
        let report = read_session_report(&path, state)?;
        if current_dir.is_some_and(|current_dir| !report.matches_current_dir(current_dir)) {
            continue;
        }
        reports.push(report);
    }

    sort_session_reports(&mut reports);
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

fn read_session_report(path: &Path, state: &AppState) -> Result<SessionReport> {
    let mut report = SessionReport::from_path(path, file_modified_epoch(path).unwrap_or(0));

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
        report.set_profile(Some(binding.profile_name.clone()));
    }
    Ok(report)
}

fn file_modified_epoch(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(prodex_core::system_time_to_unix_seconds)
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
