use super::*;

pub(crate) fn handle_audit(args: AuditArgs) -> Result<()> {
    let query = AuditLogQuery {
        tail: args.tail,
        component: args.component.as_deref().map(str::trim).map(str::to_string),
        action: args.action.as_deref().map(str::trim).map(str::to_string),
        outcome: args.outcome.as_deref().map(str::trim).map(str::to_string),
    };
    let events = read_recent_audit_events(&query)?;

    if args.json {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "audit_logs": audit_logs_json_value(),
            "filters": {
                "tail": query.tail,
                "component": query.component,
                "action": query.action,
                "outcome": query.outcome,
            },
            "events": events,
        }))
        .context("failed to serialize audit log output")?;
        println!("{json}");
        return Ok(());
    }

    println!(
        "{}",
        render_audit_events_human(&audit_log_path(), &query, &events)
    );

    Ok(())
}
