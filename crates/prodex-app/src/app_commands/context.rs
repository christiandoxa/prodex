use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::io::IsTerminal;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use terminal_ui::print_text_panel;

use super::log::{TranscriptEvent, transcript_events_from_session_line};
use crate::{
    AppPaths, AppState, AppStateIoExt, ContextAuditArgs, ContextCompactOutputArgs,
    ContextCompactOutputKind, ContextCompressArgs, ContextExportArgs, ContextReplayReportArgs,
    DEFAULT_CODEX_DIR, absolutize, current_cli_width, print_stdout_line,
};

pub(crate) use prodex_context::{
    CommandOutputCompactLimits, CommandOutputCompactOptions, CommandOutputKind,
    collect_context_audit_report, compact_command_output_with_options, compress_context_path,
    render_context_audit_report_with_width, render_context_compress_report,
};

pub(crate) fn handle_context_audit(args: ContextAuditArgs) -> Result<()> {
    let root = args.root.map(absolutize).transpose()?.unwrap_or_else(|| {
        AppPaths::discover()
            .map(|paths| paths.shared_codex_root)
            .unwrap_or_else(|_| PathBuf::from(DEFAULT_CODEX_DIR))
    });
    let report = collect_context_audit_report(&root, args.limit)?;

    if args.json {
        let json = serde_json::to_string_pretty(&report)
            .context("failed to serialize context audit report")?;
        print_stdout_line(&json);
        return Ok(());
    }

    let output = render_context_audit_report_with_width(&report, args.limit, current_cli_width());
    print_context_human_output("Context Audit", &output)?;
    Ok(())
}

pub(crate) fn handle_context_export(args: ContextExportArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let _ = prodex_session_store::repair_resume_session_metadata_prefix(
        &paths.shared_codex_root,
        &args.id,
    )?;
    let report = prodex_session_store::resolve_session_report_by_id_in_store(
        &paths.shared_codex_root,
        &state,
        &args.id,
    )
    .map_err(anyhow::Error::new)?;
    let session_path = PathBuf::from(&report.path);
    let markdown = render_session_context_export_markdown(&report, &session_path)?;
    let output_path = match args.path {
        Some(path) => absolutize(path)?,
        None => default_context_export_path(&report.id)?,
    };
    fs::write(&output_path, markdown)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    print_stdout_line(&format!(
        "Exported session context {} -> {}",
        report.id,
        output_path.display()
    ));
    Ok(())
}

pub(crate) fn handle_context_compress(args: ContextCompressArgs) -> Result<()> {
    let path = absolutize(args.path)?;
    let report = compress_context_path(&path, args.dry_run)?;

    if args.json {
        let json = serde_json::to_string_pretty(&report)
            .context("failed to serialize context compress report")?;
        print_stdout_line(&json);
    } else {
        let output = render_context_compress_report(&report, args.dry_run);
        print_context_human_output("Context Compress", &output)?;
    }
    Ok(())
}

pub(crate) fn handle_context_replay_report(args: ContextReplayReportArgs) -> Result<()> {
    let path = absolutize(args.path)?;
    let input =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let evaluation = runtime_proxy_crate::smart_context_evaluate_replay_corpus_json(&input)
        .with_context(|| {
            format!(
                "failed to evaluate Smart Context replay corpus {}",
                path.display()
            )
        })?;

    if args.json {
        let json = serde_json::to_string_pretty(&evaluation)
            .context("failed to serialize Smart Context replay evaluation")?;
        print_stdout_line(&json);
    } else {
        let report =
            runtime_proxy_crate::smart_context_render_replay_evaluation_markdown(&evaluation);
        print_context_human_output("Context Replay", &report)?;
    }

    if args.strict && !evaluation.passed {
        bail!(
            "Smart Context replay acceptance failed for {}",
            path.display()
        );
    }
    Ok(())
}

pub(crate) fn handle_context_compact_output(args: ContextCompactOutputArgs) -> Result<()> {
    let input = if let Some(path) = args.path {
        let path = absolutize(path)?;
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?
    } else {
        if io::stdin().is_terminal() {
            bail!("pass a text file path or pipe command output to stdin");
        }
        let mut input = String::new();
        io::stdin()
            .read_to_string(&mut input)
            .context("failed to read command output from stdin")?;
        input
    };
    let options = CommandOutputCompactOptions::from_limits(CommandOutputCompactLimits {
        kind: context_compact_output_kind(args.kind),
        max_lines: args.max_lines,
        head_lines: args.head_lines,
        tail_lines: args.tail_lines,
        max_line_chars: args.max_line_chars,
        max_search_matches_per_file: args.max_search_matches_per_file,
        max_path_entries: args.max_path_entries,
    });
    let report = compact_command_output_with_options(&input, &options);

    if args.json {
        let json = serde_json::to_string_pretty(&report)
            .context("failed to serialize context compact-output report")?;
        print_stdout_line(&json);
    } else if io::stdout().is_terminal() {
        print_context_human_output("Context Compact Output", report.output.trim_end())?;
    } else {
        print_stdout_line(report.output.trim_end());
    }
    Ok(())
}

fn context_compact_output_kind(kind: ContextCompactOutputKind) -> CommandOutputKind {
    match kind {
        ContextCompactOutputKind::Auto => CommandOutputKind::Auto,
        ContextCompactOutputKind::GitStatus => CommandOutputKind::GitStatus,
        ContextCompactOutputKind::GitDiff => CommandOutputKind::GitDiff,
        ContextCompactOutputKind::Search => CommandOutputKind::Search,
        ContextCompactOutputKind::FileList => CommandOutputKind::FileList,
        ContextCompactOutputKind::Plain => CommandOutputKind::Plain,
    }
}

fn print_context_human_output(title: &str, output: &str) -> Result<()> {
    print_text_panel(title, output);
    Ok(())
}

fn default_context_export_path(session_id: &str) -> Result<PathBuf> {
    Ok(env::current_dir()
        .context("failed to determine current directory")?
        .join(format!("context_{session_id}.md")))
}

fn render_session_context_export_markdown(
    report: &prodex_app_reports::SessionReport,
    session_path: &Path,
) -> Result<String> {
    let raw = fs::read_to_string(session_path)
        .with_context(|| format!("failed to read {}", session_path.display()))?;
    let mut output = String::new();
    output.push_str("# Session Context Export\n\n");
    output.push_str(&format!("- session_id: `{}`\n", report.id));
    if let Some(thread_name) = report.thread_name.as_deref() {
        output.push_str(&format!("- thread_name: {}\n", thread_name));
    }
    if let Some(updated_at) = report.updated_at.as_deref() {
        output.push_str(&format!("- updated_at: {}\n", updated_at));
    }
    if let Some(profile) = report.profile.as_deref() {
        output.push_str(&format!("- profile: `{profile}`\n"));
    }
    if let Some(provider) = report.model_provider.as_deref() {
        output.push_str(&format!("- provider: `{provider}`\n"));
    }
    if let Some(cwd) = report.cwd.as_deref() {
        output.push_str(&format!("- cwd: `{cwd}`\n"));
    }
    output.push_str(&format!("- source_path: `{}`\n\n", session_path.display()));

    let mut events = Vec::new();
    for line in raw.lines() {
        events.extend(transcript_events_from_session_line(line));
    }

    if events.is_empty() {
        output.push_str("_No transcript/context entries were found in this session file._\n");
        return Ok(output);
    }

    output.push_str("## Transcript\n\n");
    for (index, event) in events.iter().enumerate() {
        output.push_str(&render_transcript_event_markdown(index + 1, event));
    }
    Ok(output)
}

fn render_transcript_event_markdown(index: usize, event: &TranscriptEvent) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "### {}. {} · {}\n\n",
        index, event.source, event.timestamp
    ));
    output.push_str(&fenced_markdown_block(&event.text));
    output.push('\n');
    output
}

fn fenced_markdown_block(text: &str) -> String {
    let fence_len = text
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            (trimmed.starts_with('`'))
                .then_some(trimmed.chars().take_while(|ch| *ch == '`').count())
        })
        .max()
        .unwrap_or(2)
        + 1;
    let fence = "`".repeat(fence_len.max(3));
    format!("{fence}text\n{text}\n{fence}\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestEnvVarGuard;
    use prodex_app_reports::{SessionReport, apply_session_json_line};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn default_context_export_path_uses_current_directory() {
        let path = default_context_export_path("session-123").unwrap();
        assert!(path.ends_with("context_session-123.md"));
    }

    #[test]
    fn render_session_context_export_markdown_includes_metadata_and_transcript() {
        let root = env::temp_dir().join(format!(
            "prodex-context-export-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let session_path = root.join("session.jsonl");
        fs::write(
            &session_path,
            concat!(
                "{\"timestamp\":\"2026-06-20T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"base_instructions\":{\"text\":\"System prompt.\"}}}\n",
                "{\"timestamp\":\"2026-06-20T01:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"Hello model.\"}]}}\n",
                "{\"timestamp\":\"2026-06-20T01:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"exec_command\",\"arguments\":\"{\\\"cmd\\\":\\\"pwd\\\"}\"}}\n",
                "{\"timestamp\":\"2026-06-20T01:00:03Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"output\":\"/tmp/work\"}}\n",
                "{\"timestamp\":\"2026-06-20T01:00:04Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello user.\"}]}}\n"
            ),
        )
        .unwrap();

        let mut report = SessionReport::from_path(&session_path, 0);
        apply_session_json_line(
            &mut report,
            r#"{"timestamp":"2026-06-20T01:00:00Z","type":"session_meta","payload":{"thread_name":"Issue triage","cwd":"/tmp/workspace"}}"#,
        );
        report.set_profile(Some("main".to_string()));
        report.set_model_provider(Some("openai".to_string()));

        let markdown = render_session_context_export_markdown(&report, &session_path).unwrap();

        assert!(markdown.contains("# Session Context Export"));
        assert!(markdown.contains("- session_id: `session`"));
        assert!(markdown.contains("- thread_name: Issue triage"));
        assert!(markdown.contains("### 1. prompt-engineering"));
        assert!(markdown.contains("System prompt."));
        assert!(markdown.contains("### 2. user"));
        assert!(markdown.contains("Hello model."));
        assert!(markdown.contains("tool-call:exec_command"));
        assert!(markdown.contains("{\"cmd\":\"pwd\"}"));
        assert!(markdown.contains("tool-output"));
        assert!(markdown.contains("/tmp/work"));
        assert!(markdown.contains("### 5. assistant"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn handle_context_export_writes_default_markdown_in_current_directory() {
        let _env_lock = TestEnvVarGuard::lock();
        let root = env::temp_dir().join(format!(
            "prodex-context-export-handler-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let prodex_home = root.join("prodex-home");
        let shared_home = root.join("shared-codex");
        let cwd = root.join("workspace");
        let session_path = shared_home
            .join("sessions")
            .join("2026")
            .join("07")
            .join("01")
            .join("019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9.jsonl");
        fs::create_dir_all(session_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&cwd).unwrap();
        fs::write(
            &session_path,
            concat!(
                "{\"timestamp\":\"2026-06-20T01:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"id\":\"019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9\",\"thread_name\":\"Issue triage\",\"cwd\":\"/tmp/workspace\",\"base_instructions\":{\"text\":\"System prompt.\"}}}\n",
                "{\"timestamp\":\"2026-06-20T01:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"Hello model.\"}]}}\n"
            ),
        )
        .unwrap();

        let _prodex_home = TestEnvVarGuard::set("PRODEX_HOME", prodex_home.to_str().unwrap());
        let _shared_home =
            TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_home.to_str().unwrap());
        let previous_cwd = env::current_dir().unwrap();
        env::set_current_dir(&cwd).unwrap();

        let result = handle_context_export(ContextExportArgs {
            id: "019c9e3d".to_string(),
            path: None,
        });

        env::set_current_dir(previous_cwd).unwrap();
        result.unwrap();

        let exported = cwd.join("context_019c9e3d-45a0-7ad0-a6ee-b194ac2d44f9.md");
        let markdown = fs::read_to_string(&exported).unwrap();
        assert!(markdown.contains("# Session Context Export"));
        assert!(markdown.contains("Hello model."));

        fs::remove_dir_all(root).unwrap();
    }
}
