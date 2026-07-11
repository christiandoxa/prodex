use super::*;

#[test]
fn rg_json_output_groups_matches_and_skips_metadata() {
    let input = "\
{\"type\":\"begin\",\"data\":{\"path\":{\"text\":\"src/lib.rs\"}}}
{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"src/lib.rs\"},\"lines\":{\"text\":\"fn alpha() {}\\n\"},\"line_number\":10}}
{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"README.md\"},\"lines\":{\"text\":\"prodex alpha\\n\"},\"line_number\":4}}
{\"type\":\"end\",\"data\":{\"path\":{\"text\":\"src/lib.rs\"}}}
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_search_matches_per_file: 2,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(report.output.contains("sum: search matches=2, files=2"));
    assert!(report.output.contains("src/lib.rs (1 matches):"));
    assert!(report.output.contains("10: fn alpha() {}"));
    assert!(report.output.contains("README.md (1 matches):"));
    assert!(!report.output.contains("\"type\":\"begin\""));
}

#[test]
fn rg_heading_output_uses_current_file_for_numbered_matches() {
    let input = "\
src/lib.rs
10:fn alpha() {}
20:fn beta() {}
--
README.md
3:prodex alpha
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(report.output.contains("src/lib.rs (2 matches):"));
    assert!(report.output.contains("10: fn alpha() {}"));
    assert!(report.output.contains("README.md (1 matches):"));
}

#[test]
fn file_list_output_accepts_bare_paths_and_ls_listing_rows() {
    let input = "\
total 16
-rw-r--r-- 1 user group 10 May 1 12:00 Cargo.toml
-rw-r--r-- 1 user group 20 May 1 12:00 README.md
drwxr-xr-x 2 user group 4096 May 1 12:00 crates
src/main.rs
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_path_entries: 10,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::FileList);
    assert!(report.output.contains("sum: files entries=4"));
    assert!(report.output.contains("Cargo.toml"));
    assert!(report.output.contains("README.md"));
    assert!(report.output.contains("src/main.rs"));
}

#[test]
fn json_log_stream_output_summarizes_levels_and_keeps_critical_lines_exactly() {
    let mut input = String::new();
    for index in 0..25 {
        input.push_str(&format!(
            "{{\"level\":\"info\",\"message\":\"heartbeat {index}\"}}\n"
        ));
    }
    input.push_str(
        "{\"level\":\"error\",\"message\":\"database unavailable\",\"at\":\"src/db.rs:44:9\"}\n",
    );
    for index in 25..50 {
        input.push_str(&format!(
            "{{\"level\":\"info\",\"message\":\"heartbeat {index}\"}}\n"
        ));
    }

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 12,
            head_lines: 2,
            tail_lines: 2,
            max_line_chars: 180,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::LogStream);
    assert!(report.output.contains("sum: logs"));
    assert!(report.output.contains("levels: info=50, error=1"));
    assert!(report.output.contains("preserved:"));
    assert!(report.output.contains(
        "{\"level\":\"error\",\"message\":\"database unavailable\",\"at\":\"src/db.rs:44:9\"}"
    ));
    assert!(report.output.contains("database unavailable"));
    assert!(report.output.contains("src/db.rs:44:9"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn text_log_stream_preserves_warn_fatal_locations_and_stack_trace() {
    let mut input = String::new();
    for index in 0..12 {
        input.push_str(&format!(
            "2026-05-05T00:00:0{index}Z INFO worker heartbeat {index}\n"
        ));
    }
    input.push_str("2026-05-05T00:00:13Z WARN config fallback enabled\n");
    input.push_str("2026-05-05T00:00:14Z ERROR src/app.rs:77:5 request failed\n");
    input.push_str("Traceback (most recent call last):\n");
    input.push_str("  File \"src/app.py\", line 22, in handle\n");
    input.push_str("RuntimeError: boom\n");
    input.push_str("2026-05-05T00:00:15Z FATAL shutting down\n");

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 10,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::LogStream);
    assert!(report.output.contains("levels: info=12"));
    assert!(
        report
            .output
            .contains("2026-05-05T00:00:13Z WARN config fallback enabled")
    );
    assert!(
        report
            .output
            .contains("2026-05-05T00:00:14Z ERROR src/app.rs:77:5 request failed")
    );
    assert!(report.output.contains("Traceback (most recent call last):"));
    assert!(
        report
            .output
            .contains("  File \"src/app.py\", line 22, in handle")
    );
    assert!(report.output.contains("RuntimeError: boom"));
    assert!(
        report
            .output
            .contains("2026-05-05T00:00:15Z FATAL shutting down")
    );
    assert!(!report.output.contains("worker heartbeat 0"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn log_stream_compaction_keeps_stack_head_tail_and_drops_middle_noise() {
    let mut input = String::new();
    for index in 0..16 {
        input.push_str(&format!(
            "2026-05-05T00:00:{index:02}Z INFO worker heartbeat {index}\n"
        ));
    }
    input.push_str("2026-05-05T00:01:00Z ERROR request failed\n");
    input.push_str("Stack trace:\n");
    for index in 0..60 {
        input.push_str(&format!("    at middleware_frame_{index}\n"));
    }
    input.push_str("2026-05-05T00:01:01Z FATAL shutdown after failure\n");

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::LogStream,
            max_lines: 18,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::LogStream);
    assert!(report.output.contains("ERROR request failed"));
    assert!(report.output.contains("Stack trace:"));
    assert!(report.output.contains("middleware_frame_0"));
    assert!(report.output.contains("middleware_frame_59"));
    assert!(report.output.contains("omitted"));
    assert!(!report.output.contains("middleware_frame_30"));
    assert!(report.output.contains("FATAL shutdown after failure"));
    assert!(report.estimated_tokens_after < report.estimated_tokens_before / 2);
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn gh_run_watch_repeated_snapshots_compact_to_last_delta_and_failure() {
    let mut input = String::new();
    for cycle in 0..36 {
        input.push_str("Refreshing run status every 3 seconds. Press Ctrl+C to quit.\n");
        input.push_str("JOBS\n");
        match cycle {
            0..=9 => {
                input.push_str("- build queued\n");
                input.push_str("- test queued\n");
            }
            10..=20 => {
                input.push_str("* build running\n");
                input.push_str("- test queued\n");
            }
            21..=34 => {
                input.push_str("build success\n");
                input.push_str("* test running\n");
            }
            _ => {
                input.push_str("build success\n");
                input.push_str("X test failed\n");
                input.push_str("Error: Process completed with exit code 1.\n");
            }
        }
        input.push('\n');
    }

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 14,
            max_line_chars: 180,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert!(report.output.contains("pcs: watcher-delta"));
    assert!(report.output.contains("last state changes"));
    assert!(report.output.contains("X test failed"));
    assert!(
        report
            .output
            .contains("Error: Process completed with exit code 1.")
    );
    assert!(!report.output.contains("- build queued"));
    assert!(report.compacted_lines <= 14);
    assert_token_regression_budget(&input, &report, 70);
}

#[test]
fn successful_command_output_one_lines_noisy_success_and_keeps_failures_exact() {
    let mut input = String::new();
    for index in 0..80 {
        input.push_str(&format!(
            "Progress: resolved {}, reused {}, downloaded 0, added 0\n",
            index + 1,
            index
        ));
    }
    input.push_str("Done in 3.21s using pnpm v9.0.0\n");

    let report = compact_successful_command_output_with_options(
        &input,
        &CommandSuccessOutputCompactOptions {
            command: Some("pnpm install".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 10,
            max_line_chars: 180,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert_eq!(report.compacted_lines, 1);
    assert!(report.output.contains("pcs: ok lines=81"));
    assert!(report.output.contains("noise:"));
    assert!(report.output.contains("cmd: pnpm install"));
    assert!(!report.output.contains("Progress: resolved 80"));
    assert_no_critical_signal_loss(&input, &report.output);

    let failure = "\
PASS tests/unit.test.ts
Error: Process completed with exit code 1.
src/app.ts:12:7
";
    let failure_report = compact_successful_command_output_with_options(
        failure,
        &CommandSuccessOutputCompactOptions {
            command: Some("pnpm test".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(!failure_report.compacted);
    assert!(failure_report.failure_suspected);
    assert_eq!(failure_report.output, failure);
}

#[test]
fn github_actions_failure_log_slices_failed_step_exit_and_error_context() {
    let mut input = String::new();
    input.push_str("Current runner version: '2.329.0'\n");
    input.push_str("job: test-linux\n");
    input.push_str("##[group]Run actions/checkout@v4\n");
    input.push_str("Syncing repository: acme/prodex\n");
    input.push_str("##[endgroup]\n");
    for index in 0..20 {
        input.push_str(&format!("INFO cache restore line {index}\n"));
    }
    input.push_str("##[group]Run cargo test --workspace\n");
    input.push_str("cargo test --workspace --locked\n");
    for index in 0..30 {
        input.push_str(&format!("   Compiling crate_{index} v0.1.0\n"));
    }
    input.push_str("error[E0425]: cannot find value `missing` in this scope\n");
    input.push_str("  --> crates/app/src/lib.rs:88:13\n");
    input.push_str("   |\n");
    input.push_str("88 |     missing();\n");
    input.push_str("   |     ^^^^^^^ not found in this scope\n");
    input.push_str(
        "process didn't exit successfully: `cargo test --workspace` (exit status: 101)\n",
    );
    input.push_str("##[error]Process completed with exit code 101.\n");

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 18,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert!(report.output.contains("pcs: ci-failure"));
    assert!(report.output.contains("job=test-linux"));
    assert!(report.output.contains("step=cargo test --workspace"));
    assert!(report.output.contains("exit=101"));
    assert!(
        report
            .output
            .contains("error[E0425]: cannot find value `missing`")
    );
    assert!(report.output.contains("crates/app/src/lib.rs:88:13"));
    assert!(
        report
            .output
            .contains("process didn't exit successfully: `cargo test --workspace`")
    );
    assert!(
        report
            .output
            .contains("##[error]Process completed with exit code 101.")
    );
    assert!(!report.output.contains("INFO cache restore line 0"));
    assert!(!report.output.contains("Compiling crate_0"));
    assert_token_regression_budget(&input, &report, 65);
}

#[test]
fn token_regression_harness_guards_noisy_failure_compaction() {
    let mut input = String::from(
        "\
> app@1.0.0 test /repo
",
    );
    for index in 0..90 {
        input.push_str(&format!(
            "PASS tests/noisy_{index}.test.ts ({} ms)\n",
            index + 10
        ));
    }
    input.push_str(
        "\
FAIL tests/payment.test.ts
AssertionError: expected status 201, received 500
    at createPayment (/repo/src/payment.ts:88:13)
    at Object.<anonymous> (/repo/tests/payment.test.ts:42:5)
Test Suites: 1 failed, 90 passed, 91 total
Tests:       1 failed, 270 passed, 271 total
Command failed with exit code 1
",
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 24,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("FAIL tests/payment.test.ts"));
    assert!(
        report
            .output
            .contains("AssertionError: expected status 201")
    );
    assert!(report.output.contains("src/payment.ts:88:13"));
    assert!(report.output.contains("Command failed with exit code 1"));
    assert!(!report.output.contains("PASS tests/noisy_89.test.ts"));
    assert_token_regression_budget(&input, &report, 70);
}
