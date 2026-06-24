use anyhow::{Context, Result, bail};
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

    print_stdout_line(&render_context_audit_report_with_width(
        &report,
        args.limit,
        current_cli_width(),
    ));
    Ok(())
}

pub(crate) fn handle_context_compress(args: ContextCompressArgs) -> Result<()> {
    let path = absolutize(args.path)?;
    let report = compress_context_path(&path, args.dry_run)?;

    let json = if args.json {
        serde_json::to_string_pretty(&report)
            .context("failed to serialize context compress report")?
    } else {
        render_context_compress_report(&report, args.dry_run)
    };
    print_stdout_line(&json);
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
        print_stdout_line(&report);
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
