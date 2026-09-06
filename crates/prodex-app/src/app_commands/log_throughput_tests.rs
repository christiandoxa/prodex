use super::log_stream::{LogStreamItem, collect_runtime_log_line};
use crate::app_commands::log_tui::{
    LOG_TUI_TITLE, OutputThroughput, OutputThroughputDisplay, render_log_header,
    render_log_header_with_display,
};
use std::collections::VecDeque;
use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

#[test]
fn completed_rate_reaches_tui_header_and_survives_log_flood() {
    let path = Path::new("/home/test-user/runtime-throughput-tui.log");
    let mut throughput = OutputThroughput::default();
    let initial = render_log_header(
        LOG_TUI_TITLE,
        "",
        None,
        throughput.display_rate_for_profile(Instant::now(), Some("main")),
        80,
    );
    assert!(initial.ends_with("— t/s"));

    let completed = collect_runtime_log_line(
        path,
        "[2026-07-01 21:52:36.729 +07:00] token_usage request=30 route=responses transport=http profile=main source=responses_sse input_tokens=10 cached_input_tokens=0 output_tokens=100 reasoning_tokens=0 generation_ms=1000 output_tokens_per_second=100.0",
        false,
        Some(&mut throughput),
        true,
    )
    .unwrap();
    assert!(matches!(
        completed.as_slice(),
        [LogStreamItem::TokenUsage(_)]
    ));

    for _ in 0..1_000 {
        collect_runtime_log_line(
            path,
            "[2026-07-01 21:52:36.730 +07:00] runtime_proxy_active_limit_reached route=responses profile=main active=8 limit=8",
            true,
            Some(&mut throughput),
            true,
        )
        .unwrap();
    }

    let rate = throughput.display_rate_for_profile(Instant::now(), Some("main"));
    let display = throughput.display_for_profile(Instant::now(), Some("main"));
    assert_eq!(rate, Some(100.0));
    assert_eq!(display, Some(OutputThroughputDisplay::Last(100.0)));
    let rendered = render_log_header_with_display(LOG_TUI_TITLE, "", None, display, 80);
    assert!(rendered.contains("last 100 t/s"));
    assert!(render_log_header(LOG_TUI_TITLE, "", None, rate, 80).contains("last 100 t/s"));
}

#[test]
fn log_display_latency_fixture_keeps_p95_below_250ms() {
    const SAMPLES: usize = 128;
    const SLOW_SUBSCRIBER_BUFFER: usize = 64;
    let paths = [
        Path::new("/home/test-user/runtime-log-display-fixture-a.log"),
        Path::new("/home/test-user/runtime-log-display-fixture-b.log"),
    ];
    let mut throughput = OutputThroughput::default();
    let mut latencies = Vec::with_capacity(SAMPLES);

    for sample in 0..SAMPLES {
        let started_at = Instant::now();
        let mut items = VecDeque::new();
        let path = paths[(sample / 32) % paths.len()];
        let profile = if sample % 2 == 0 { "main" } else { "backup" };
        for line in [
            format!(
                "[2026-09-06 10:00:00.000 +00:00] selection_pick request={} profile={} route=responses model=fixture-model",
                sample + 1,
                profile
            ),
            format!(
                "[2026-09-06 10:00:00.001 +00:00] stream_payload request=1 route=websocket transport=websocket profile={profile} source=assistant stream=\"hello 🌋\""
            ),
            format!(
                "[2026-09-06 10:00:00.002 +00:00] token_usage request=1 route=responses transport=http profile={profile} source=responses_sse input_tokens=10 output_tokens=100 reasoning_tokens=0 generation_ms=1000"
            ),
            format!(
                "[2026-09-06 10:00:00.003 +00:00] terminal_event request=1 route=responses transport=http profile={profile} event_type=response.completed status=200"
            ),
        ] {
            for item in
                collect_runtime_log_line(path, &line, true, Some(&mut throughput), false).unwrap()
            {
                items.push_back(item);
                while items.len() > SLOW_SUBSCRIBER_BUFFER {
                    items.pop_front();
                }
            }
        }
        if sample % 16 == 0 {
            for backlog in 0..(SLOW_SUBSCRIBER_BUFFER - 8) {
                items.push_back(LogStreamItem::Transcript(
                    crate::app_commands::TranscriptEvent {
                        timestamp: "2026-09-06 10:00:00.004 +00:00".to_string(),
                        source: "event".to_string(),
                        text: format!("slow-subscriber-backlog-{backlog}"),
                    },
                ));
            }
            while items.len() > SLOW_SUBSCRIBER_BUFFER {
                items.pop_front();
            }
        }
        items.push_back(LogStreamItem::Transcript(
            crate::app_commands::TranscriptEvent {
                timestamp: "2026-09-06 10:00:00.004 +00:00".to_string(),
                source: "assistant".to_string(),
                text: "large transcript 🌋 ".repeat(4_096),
            },
        ));
        while items.len() > SLOW_SUBSCRIBER_BUFFER {
            items.pop_front();
        }
        let display = throughput.display_for_profile(Instant::now(), Some(profile));
        let header = render_log_header_with_display(LOG_TUI_TITLE, "", None, display, 120);
        let rendered = super::log_stream_tui_text(&items, 1, 120);
        black_box((header, rendered));
        latencies.push(started_at.elapsed());
    }

    latencies.sort_unstable();
    let p95 = latencies[(SAMPLES * 95).div_ceil(100).saturating_sub(1)];
    eprintln!(
        "log_display_latency_fixture sample_count={SAMPLES} p95_ms={} os={} arch={}",
        p95.as_secs_f64() * 1_000.0,
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    assert!(
        p95 <= Duration::from_millis(250),
        "log display p95 exceeded 250ms: {p95:?}"
    );
}
