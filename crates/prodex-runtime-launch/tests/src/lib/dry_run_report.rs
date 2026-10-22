use crate::{
    ChildProcessPlan, RuntimeLaunchPlan, RuntimeProxyCodexEndpoint, runtime_launch_dry_run_report,
};
use std::ffi::OsString;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

fn runtime_launch_report_corpus() -> String {
    let home = std::env::temp_dir().join(format!(
        "prodex-launch-report-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    assert!(!home.exists());
    let child = || ChildProcessPlan::new(OsString::from("codex"), home.clone());
    let proxy = |listen_addr: SocketAddr, openai_mount_path| RuntimeProxyCodexEndpoint {
        listen_addr,
        openai_mount_path,
        local_model_provider_id: None,
        force_http_responses: false,
        realtime_ws_base_url: None,
        realtime_ws_model: None,
    };
    let cases = vec![
        ("defaults", child(), None),
        (
            "configured-ordered-empty",
            child()
                .with_args(
                    [
                        "-c",
                        "model_provider=\"synthetic-provider\"",
                        "-c",
                        "model=\"synthetic-model\"",
                        "",
                        "--",
                        "",
                    ]
                    .into_iter()
                    .map(OsString::from)
                    .collect(),
                )
                .with_extra_env([
                    ("", OsString::from("")),
                    ("VISIBLE", OsString::from("first")),
                    ("VISIBLE", OsString::from("second")),
                ])
                .with_removed_env(["OLD", "", "OLD"]),
            None,
        ),
        (
            "empty-present",
            child().with_args(
                ["-c", "model_provider=\"\"", "-c", "model=\"\""]
                    .into_iter()
                    .map(OsString::from)
                    .collect(),
            ),
            None,
        ),
        (
            "proxy-would",
            child(),
            Some(proxy(SocketAddr::from(([127, 0, 0, 1], 0)), "/codex")),
        ),
        (
            "proxy-enabled-redacted",
            child()
                .with_args(
                    [
                        "--api-key",
                        "synthetic-launch-secret",
                        "-c",
                        "api_key=\"synthetic-launch-secret\"",
                    ]
                    .into_iter()
                    .map(OsString::from)
                    .collect(),
                )
                .with_extra_env([("AUTH_TOKEN", OsString::from("synthetic-launch-secret"))]),
            Some(proxy(SocketAddr::from(([127, 0, 0, 1], 443)), "/v1")),
        ),
        (
            "proxy-ipv6-unicode",
            child()
                .with_args(vec![
                    OsString::from("--model"),
                    OsString::from(" \t雪\"\\\n "),
                ])
                .with_extra_env([("VISIBLE", OsString::from(" \t雪\"\\\n "))]),
            Some(proxy(
                SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 1234)),
                "/v1",
            )),
        ),
        (
            "long",
            child()
                .with_args(vec![OsString::from("x".repeat(4097))])
                .with_extra_env([("VISIBLE", OsString::from("e".repeat(4097)))])
                .with_removed_env(["R".repeat(4097)]),
            None,
        ),
    ];
    let mut corpus = String::new();
    for (name, child, proxy) in cases {
        let report = runtime_launch_dry_run_report(
            &format!("flow-{name}"),
            &home,
            proxy,
            &RuntimeLaunchPlan::new(child),
        )
        .unwrap();
        assert!(!report.contains("synthetic-launch-secret"));
        assert!(!report.contains(home.to_string_lossy().as_ref()));
        corpus.push_str(&format!("=== {name} ===\n{report}"));
    }
    corpus
}

#[test]
fn runtime_launch_reports_preserve_original_complete_text() {
    let actual = runtime_launch_report_corpus()
        .split_inclusive('\n')
        .map(|line| format!("{line:?}"))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        include_str!("dry_run_report_baseline.txt")
            .lines()
            .collect::<Vec<_>>()
    );
}
