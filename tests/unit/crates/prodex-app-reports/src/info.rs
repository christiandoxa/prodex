use super::*;

#[test]
fn token_usage_summary_parses_text_runtime_log_markers() {
    let summary = collect_info_token_usage_summary_from_text(concat!(
        "[2026-04-29 10:00:00.000 +07:00] token_usage request=1 transport=http profile=main source=responses_unary input_tokens=100 cached_input_tokens=25 output_tokens=40 reasoning_tokens=8\n",
        "[2026-04-29 10:00:01.000 +07:00] token_usage request=2 transport=http profile=backup source=responses_sse input_tokens=10 cached_input_tokens=0 output_tokens=4 reasoning_tokens=1\n",
    ));

    assert_eq!(summary.event_count, 2);
    assert_eq!(summary.total.input_tokens, 110);
    assert_eq!(summary.total.cached_input_tokens, 25);
    assert_eq!(summary.total.output_tokens, 44);
    assert_eq!(summary.total.reasoning_tokens, 9);
    assert_eq!(summary.by_profile["main"].total.output_tokens, 40);
}

#[test]
fn token_usage_summary_parses_json_runtime_log_markers() {
    let summary = collect_info_token_usage_summary_from_text(
        r#"{"timestamp":"2026-04-29 10:00:00.000 +07:00","message":"token_usage request=1 transport=http profile=main source=responses_unary input_tokens=100 cached_input_tokens=25 output_tokens=40 reasoning_tokens=8","event":"token_usage","fields":{"request":"1","transport":"http","profile":"main","source":"responses_unary","input_tokens":"100","cached_input_tokens":"25","output_tokens":"40","reasoning_tokens":"8"}}"#,
    );

    assert_eq!(summary.event_count, 1);
    assert_eq!(summary.total.input_tokens, 100);
    assert!(format_info_token_usage_summary(&summary).contains("input=100"));
}

#[test]
fn active_runtime_log_paths_filter_to_runtime_processes() {
    let processes = vec![
        ProdexProcessInfo {
            pid: 200,
            runtime: true,
        },
        ProdexProcessInfo {
            pid: 201,
            runtime: false,
        },
    ];
    let paths = select_active_runtime_log_paths(
        &processes,
        [
            PathBuf::from("/tmp/prodex-runtime-200-old.log"),
            PathBuf::from("/tmp/prodex-runtime-201-ignored.log"),
            PathBuf::from("/tmp/prodex-runtime-200-new.log"),
        ],
    );

    assert_eq!(
        paths,
        vec![PathBuf::from("/tmp/prodex-runtime-200-new.log")]
    );
}

#[test]
fn recent_runtime_log_paths_sort_by_modified_then_path_descending() {
    let base = std::time::UNIX_EPOCH;
    let paths = select_recent_runtime_log_paths(
        [
            (
                PathBuf::from("/tmp/prodex-runtime-100-a.log"),
                base + std::time::Duration::from_secs(5),
            ),
            (
                PathBuf::from("/tmp/prodex-runtime-100-b.log"),
                base + std::time::Duration::from_secs(5),
            ),
            (
                PathBuf::from("/tmp/prodex-runtime-100-c.log"),
                base + std::time::Duration::from_secs(1),
            ),
        ],
        2,
    );

    assert_eq!(
        paths,
        vec![
            PathBuf::from("/tmp/prodex-runtime-100-b.log"),
            PathBuf::from("/tmp/prodex-runtime-100-a.log"),
        ]
    );
}

#[test]
fn runtime_info_summary_parts_preserve_display_and_json_shapes() {
    assert_eq!(
        format_runtime_policy_summary(Some("/tmp/policy.toml"), Some(1)),
        "/tmp/policy.toml (v1)"
    );
    assert_eq!(format_runtime_policy_summary(None, None), "disabled");
    assert_eq!(
        runtime_policy_json_value(Some("/tmp/policy.toml"), Some(1)),
        serde_json::json!({"path": "/tmp/policy.toml", "version": 1})
    );
    assert_eq!(
        runtime_policy_json_value(None, None),
        serde_json::Value::Null
    );
    assert_eq!(
        format_runtime_logs_summary("/tmp/prodex", "json"),
        "/tmp/prodex (json)"
    );
    assert_eq!(
        runtime_logs_json_value("/tmp/prodex", "json"),
        serde_json::json!({"directory": "/tmp/prodex", "format": "json"})
    );
}

#[test]
fn secret_backend_summary_parts_preserve_display_and_json_shapes() {
    assert_eq!(
        format_secret_backend_summary_parts(Some("keyring"), Some("prodex"), None),
        "keyring (prodex)"
    );
    assert_eq!(
        secret_backend_json_value_parts(Some("file"), None, None),
        serde_json::json!({"backend": "file", "keyring_service": null})
    );
    assert_eq!(
        format_secret_backend_summary_parts(None, None, Some("bad config")),
        "invalid (bad config)"
    );
    assert_eq!(
        secret_backend_json_value_parts(None, None, Some("bad config")),
        serde_json::json!({"invalid": true, "error": "bad config"})
    );
}
