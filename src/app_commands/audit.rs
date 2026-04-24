use super::*;

pub(crate) fn handle_audit(args: AuditArgs) -> Result<()> {
    let query = AuditLogQuery {
        tail: args.tail,
        component: args.component.as_deref().map(str::trim).map(str::to_string),
        action: args.action.as_deref().map(str::trim).map(str::to_string),
        outcome: args.outcome.as_deref().map(str::trim).map(str::to_string),
    };
    let result = read_recent_audit_events_with_scope(&query)?;

    if args.json {
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "audit_logs": audit_logs_json_value(),
            "search_scope": result.search_scope,
            "filters": {
                "tail": query.tail,
                "component": query.component,
                "action": query.action,
                "outcome": query.outcome,
            },
            "events": result.events,
        }))
        .context("failed to serialize audit log output")?;
        print_stdout_line(&json);
        return Ok(());
    }

    print_stdout_line(&render_audit_events_human_with_scope(
        &result.search_scope.path,
        &query,
        &result.events,
        Some(&result.search_scope),
    ));

    Ok(())
}
