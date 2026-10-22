use super::super::{PingResult, PingStatus};
use super::{render_ping_result, render_ping_summary};
use std::time::Duration;

fn sample(profile: &str, status: PingStatus) -> PingResult {
    PingResult {
        profile: profile.to_string(),
        model: None,
        effort: None,
        effective_model: None,
        status,
        detail: "synthetic detail".to_string(),
        first_response_latency_ms: None,
        latency_ms: None,
    }
}

fn capture(name: &str, render: impl FnOnce() -> anyhow::Result<()>) {
    println!("PING_REPORT_BEGIN {name}");
    render().unwrap();
    println!("PING_REPORT_END {name}");
}

#[test]
fn ping_report_capture_fixture_child() {
    let missing = sample("synthetic-missing", PingStatus::Pass);
    capture("row-missing", || render_ping_result(&missing));
    let mut unicode = sample("profile-雪", PingStatus::AuthFailed);
    unicode.model = Some("model-\"雪".into());
    unicode.effort = Some("".into());
    unicode.effective_model = Some("effective\nmodel".into());
    unicode.detail = "synthetic reason\n雪\tquoted\"".into();
    unicode.first_response_latency_ms = Some(0);
    unicode.latency_ms = Some(u64::MAX as u128);
    capture("row-unicode", || render_ping_result(&unicode));
    let mut wide = unicode.clone();
    wide.first_response_latency_ms = Some(u128::MAX);
    wide.latency_ms = Some(u128::MAX);
    capture("row-u128", || render_ping_result(&wide));
    let statuses = [
        PingStatus::Pass,
        PingStatus::AuthFailed,
        PingStatus::DnsFailed,
        PingStatus::TlsFailed,
        PingStatus::Timeout,
        PingStatus::RateLimited,
        PingStatus::QuotaExhausted,
        PingStatus::UpstreamOverloaded,
        PingStatus::ModelUnavailable,
        PingStatus::ProtocolFailed,
        PingStatus::TurnFailed,
        PingStatus::ProcessFailed,
        PingStatus::SpawnFailed,
        PingStatus::Cancelled,
        PingStatus::UnexpectedResponse,
    ];
    let mixed = statuses
        .into_iter()
        .enumerate()
        .map(|(index, status)| sample(&format!("synthetic-{index}"), status))
        .collect::<Vec<_>>();
    capture("row-all-statuses", || {
        for result in &mixed {
            render_ping_result(result)?;
        }
        Ok(())
    });
    capture("empty-human", || {
        render_ping_summary(&[], Duration::ZERO, false)
    });
    capture("empty-json", || {
        render_ping_summary(&[], Duration::ZERO, true)
    });
    let mut first = sample("profile-雪", PingStatus::Pass);
    first.model = Some("requested\"雪".into());
    first.effort = Some("".into());
    first.effective_model = Some("effective\n雪".into());
    first.first_response_latency_ms = Some(0);
    first.latency_ms = Some(u64::MAX as u128);
    let healthy = [first, missing];
    capture("healthy-json", || {
        render_ping_summary(&healthy, Duration::from_millis(1234), true)
    });
    capture("mixed-human", || {
        render_ping_summary(&mixed, Duration::from_millis(1234), false)
    });
    capture("mixed-json", || {
        render_ping_summary(&mixed, Duration::from_millis(1234), true)
    });
    let failed = [
        unicode,
        sample("synthetic-quota", PingStatus::QuotaExhausted),
    ];
    capture("failed-wide-human", || {
        render_ping_summary(&failed, Duration::MAX, false)
    });
    capture("failed-u64-json", || {
        render_ping_summary(&failed, Duration::from_millis(u64::MAX), true)
    });
}

#[test]
fn ping_report_preserves_original_complete_output() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "ping_report_capture_fixture_child",
            "--quiet",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let mut actual = Vec::new();
    let mut current = None;
    let mut body = String::new();
    for line in text.split_inclusive('\n') {
        if let Some(name) = line.strip_prefix("PING_REPORT_BEGIN ") {
            assert!(current.is_none());
            current = Some(name.trim_end_matches('\n').to_string());
        } else if let Some(name) = line.strip_prefix("PING_REPORT_END ") {
            let name = name.trim_end_matches('\n');
            assert_eq!(current.take().as_deref(), Some(name));
            actual.push(serde_json::json!({"name": name, "output": std::mem::take(&mut body)}));
        } else if current.is_some() {
            body.push_str(line);
        }
    }
    assert!(current.is_none());
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("report_baseline.json")).unwrap();
    assert_eq!(serde_json::Value::Array(actual), expected);
}
