use super::*;

pub(crate) use prodex_context::{
    collect_context_audit_report, compress_context_path, render_context_audit_report_with_width,
    render_context_compress_report,
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
