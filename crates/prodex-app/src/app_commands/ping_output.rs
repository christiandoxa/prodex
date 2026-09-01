use super::{PingResult, PingStatus};
use anyhow::Result;
use std::time::Duration;
use terminal_ui::print_stdout_line;

pub(super) fn render_ping_result(result: &PingResult) -> Result<()> {
    print_stdout_line(&format!(
        "{}  {:<20} {:>6}ms  {}",
        result.profile,
        result.status.label(),
        result.latency_ms.unwrap_or_default(),
        result.model.as_deref().unwrap_or("configured/default")
    ))?;
    if result.status.is_failure() {
        print_stdout_line(&format!("  reason: {}", result.detail))?;
    }
    Ok(())
}

pub(super) fn render_ping_summary(
    results: &[PingResult],
    elapsed: Duration,
    json: bool,
) -> Result<()> {
    let total = results.len();
    let healthy = results
        .iter()
        .filter(|result| result.status == PingStatus::Pass)
        .count();
    let exhausted = results
        .iter()
        .filter(|result| result.status == PingStatus::QuotaExhausted)
        .count();
    let auth_failures = results
        .iter()
        .filter(|result| result.status == PingStatus::AuthFailed)
        .count();
    let temporary_failures = results
        .iter()
        .filter(|result| result.status.is_temporary())
        .count();
    let other_failures =
        total.saturating_sub(healthy + exhausted + auth_failures + temporary_failures);
    let pool_usable = healthy > 0;
    if json {
        let value = serde_json::json!({
            "provider": "openai",
            "status": if healthy == total && total > 0 { "ok" } else { "failed" },
            "model": results.first().and_then(|result| result.model.clone()),
            "latency_ms": elapsed.as_millis(),
            "detail": format!("{healthy}/{total} profiles healthy"),
            "profiles": results.iter().map(|result| serde_json::json!({
                "profile": result.profile,
                "status": result.status.json_label(),
                "model": result.model,
                "latency_ms": result.latency_ms,
                "detail": result.detail,
            })).collect::<Vec<_>>(),
            "summary": {
                "profiles_discovered": total,
                "profiles_tested": total,
                "healthy": healthy,
                "exhausted": exhausted,
                "auth_failures": auth_failures,
                "temporary_failures": temporary_failures,
                "other_failures": other_failures,
                "duration_ms": elapsed.as_millis(),
                "pool_usable": pool_usable,
            },
        });
        print_stdout_line(&serde_json::to_string(&value)?)?;
        return Ok(());
    }
    print_stdout_line(&format!("Profiles discovered: {total}"))?;
    print_stdout_line(&format!("Profiles tested: {total}"))?;
    print_stdout_line(&format!("Healthy: {healthy}"))?;
    print_stdout_line(&format!("Exhausted: {exhausted}"))?;
    print_stdout_line(&format!("Auth failures: {auth_failures}"))?;
    print_stdout_line(&format!("Temporary failures: {temporary_failures}"))?;
    print_stdout_line(&format!("Other failures: {other_failures}"))?;
    print_stdout_line(&format!("Total duration: {}ms", elapsed.as_millis()))?;
    print_stdout_line(&format!(
        "Pool usable: {}",
        if pool_usable { "yes" } else { "no" }
    ))?;
    Ok(())
}
