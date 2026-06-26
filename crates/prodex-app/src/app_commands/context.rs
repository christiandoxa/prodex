use anyhow::{Context, Result, bail};
use crossterm::terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::fs;
use std::io::IsTerminal;
use std::io::{self, Read};
use std::path::PathBuf;

use crate::{
    AppPaths, ContextAuditArgs, ContextCompactOutputArgs, ContextCompactOutputKind,
    ContextCompressArgs, ContextReplayReportArgs, DEFAULT_CODEX_DIR, absolutize, current_cli_width,
    print_stdout_line,
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
    let height = context_tui_height(output);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        print_stdout_line(output);
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::styled(
            title.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(context_tui_text(output))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn context_tui_height(output: &str) -> u16 {
    let rows = output.lines().count().saturating_add(4).max(8);
    let terminal_height = terminal::size()
        .map(|(_, height)| usize::from(height))
        .unwrap_or(24);
    rows.min(terminal_height).max(1) as u16
}

fn context_tui_text(output: &str) -> Text<'static> {
    Text::from(
        output
            .lines()
            .map(|line| Line::from(context_tui_spans(line)))
            .collect::<Vec<_>>(),
    )
}

fn context_tui_spans(line: &str) -> Vec<Span<'static>> {
    if line.starts_with('#') || line.starts_with("== ") {
        return vec![Span::styled(
            line.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )];
    }
    let lower = line.to_ascii_lowercase();
    if lower.contains("failed") || lower.contains("error") || lower.contains("over budget") {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(Color::Red),
        )];
    }
    if lower.contains("warning") || lower.contains("duplicate") || lower.contains("truncated") {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(Color::Yellow),
        )];
    }
    if lower.contains("passed") || lower.contains("saved") || lower.contains("compressed") {
        return vec![Span::styled(
            line.to_string(),
            Style::default().fg(Color::Green),
        )];
    }
    vec![Span::raw(line.to_string())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_tui_text_contains_output() {
        let text = format!("{:?}", context_tui_text("# Context\ncompressed output"));
        assert!(text.contains("Context"));
        assert!(text.contains("compressed output"));
    }

    #[test]
    fn context_tui_height_is_positive() {
        assert!(context_tui_height("one\ntwo") >= 1);
    }
}
