use super::*;

#[test]
fn critical_signal_counts_mixed_command_output() {
    let input = "\
\u{1b}[31merror[E0308]: mismatched types\u{1b}[0m
  --> crates/prodex-context/src/lib.rs:42:17
   = note: expected `usize`, found `&str`
@@ -1,3 +1,4 @@
test tests::critical_signal_failure ... FAILED
---- tests::critical_signal_failure stdout ----
thread 'tests::critical_signal_failure' panicked at crates/prodex-context/tests/src/lib.rs:211:9:
stack backtrace:
             at /rustc/library/core/src/panicking.rs:75:14
test result: FAILED. 0 passed; 1 failed; finished in 0.01s
process didn't exit successfully: `cargo test` (exit status: 101)
";

    let counts = count_critical_signals(input);

    assert_eq!(counts.errors, 2);
    assert_eq!(counts.file_locations, 3);
    assert_eq!(counts.diff_hunks, 1);
    assert_eq!(counts.test_failures, 3);
    assert_eq!(counts.exit_codes, 1);
    assert_eq!(counts.stack_markers, 1);
    assert_eq!(counts.rust_diagnostics, 3);
    assert_eq!(counts.total(), 14);
}

#[test]
fn critical_signal_counts_proxy_payload_error_text() {
    let input =
        r#"{"error":{"message":"upstream failed","type":"server_error","code":"internal_error"}}"#
            .to_string()
            + "\nrequest=abc status=error upstream_status=500\n"
            + "src/runtime_proxy.rs:88:13: forwarded payload marker\n";

    let counts = count_critical_signals(&input);

    assert_eq!(counts.errors, 2);
    assert_eq!(counts.file_locations, 1);
    assert_eq!(counts.diff_hunks, 0);
    assert_eq!(counts.test_failures, 0);
    assert_eq!(counts.exit_codes, 0);
    assert_eq!(counts.stack_markers, 0);
    assert_eq!(counts.rust_diagnostics, 0);
}

#[test]
fn critical_signal_self_check_reports_lost_and_gained_counts() {
    let before = "\
error: build failed
  --> src/lib.rs:7:5
test tests::boom ... FAILED
process exited with exit code 101
";
    let after = "\
error: build failed
@@ -7,1 +7,1 @@
";

    let check = critical_signal_self_check(before, after);

    assert!(check.has_loss());
    assert!(!check.passed());
    assert_eq!(check.before.errors, 1);
    assert_eq!(check.after.errors, 1);
    assert_eq!(check.lost.file_locations, 1);
    assert_eq!(check.lost.test_failures, 1);
    assert_eq!(check.lost.exit_codes, 1);
    assert_eq!(check.gained.diff_hunks, 1);
}

#[test]
fn critical_signal_self_check_passes_when_signal_counts_preserved() {
    let before = "\
running 2 tests
test tests::boom ... FAILED
thread 'tests::boom' panicked at src/lib.rs:7:5:
stack backtrace:
process didn't exit successfully: `cargo test` (exit status: 101)
";
    let after = "\
test tests::boom ... FAILED
thread 'tests::boom' panicked at src/lib.rs:7:5:
stack backtrace:
exit status: 101
";

    let check = critical_signal_self_check(before, after);

    assert!(check.passed());
    assert!(!check.has_loss());
    assert_eq!(check.lost.total(), 0);
    assert_eq!(check.before.total(), check.after.total());
}

#[test]
fn generated_compaction_header_detection_accepts_old_and_short_labels() {
    for line in [
        "# prodex context saver: rust diagnostics (100 -> 10 lines)",
        "pcs: rust-diag (100->10)",
        "rust/cargo summary: errors=0",
        "diagnostic summary: errors=0",
        "success output summary: noisy_success_lines=12",
        "command output summary: 100 lines, 4 critical lines preserved",
        "sum: rust errors=0",
        "sum: diag errors=0",
        "sum: success noisy=12",
        "baseline compaction:",
        "base:",
        "intent matches: 2 lines for runtime",
        "int: 2 lines for runtime",
    ] {
        assert!(
            is_generated_compaction_header_line(line),
            "label should be generated: {line}"
        );
    }
}

#[test]
fn critical_signal_lost_line_ranges_find_small_windows() {
    let before = "\
setup
error: hidden failure
src/main.rs:22:5
details
test tests::boom ... FAILED
tail
";
    let after = "\
setup
summary without critical output
";

    let ranges = critical_signal_lost_line_ranges_with_options(
        before,
        after,
        CriticalSignalLineRangeOptions {
            context_lines: 1,
            max_ranges: 8,
            max_range_lines: 4,
        },
    );

    assert_eq!(
        ranges,
        vec![
            CriticalSignalLineRange { start: 1, end: 4 },
            CriticalSignalLineRange { start: 4, end: 6 },
        ]
    );
}

#[test]
fn critical_signal_lost_line_ranges_respect_after_duplicates() {
    let before = "\
error: same failure
noise
error: same failure
";
    let after = "error: same failure\n";

    let ranges = critical_signal_lost_line_ranges_with_options(
        before,
        after,
        CriticalSignalLineRangeOptions {
            context_lines: 0,
            max_ranges: 8,
            max_range_lines: 1,
        },
    );

    assert_eq!(ranges, vec![CriticalSignalLineRange { start: 3, end: 3 }]);
}

#[test]
fn critical_signal_lost_line_ranges_empty_when_counts_preserved() {
    let before = "\
error: build failed
src/main.rs:22:5
";
    let after = "\
error: build failed
src/main.rs:22:5
";

    assert!(critical_signal_lost_line_ranges(before, after).is_empty());
}

#[test]
fn context_static_duplicate_report_detects_repeated_snippets_across_roots() {
    let root = temp_context_root("static-dupes");
    std::fs::create_dir_all(root.join("rules")).expect("rules dir should be created");
    std::fs::create_dir_all(root.join("skills/review")).expect("skill dir should be created");
    let duplicate = "Always preserve upstream request metadata unless it is hop-by-hop or authentication metadata that must be replaced for the selected profile.";
    std::fs::write(
        root.join("AGENTS.md"),
        format!("# Instructions\n\n{duplicate}\n\nUnique AGENTS guidance lives here.\n"),
    )
    .expect("AGENTS should be written");
    std::fs::write(
        root.join("rules/runtime.md"),
        format!("# Runtime Rules\n\n{duplicate}\n"),
    )
    .expect("rules file should be written");
    std::fs::write(
        root.join("skills/review/SKILL.md"),
        format!("---\nname: review\n---\n\n{duplicate}\n"),
    )
    .expect("skill file should be written");

    let report =
        collect_context_static_duplicate_report(&root, 10).expect("duplicate report should load");
    let snippet = report
        .snippets
        .first()
        .expect("duplicate snippet should be reported");
    let paths = snippet
        .occurrences
        .iter()
        .map(|occurrence| occurrence.relative_path.as_str())
        .collect::<Vec<_>>();

    assert_eq!(report.total_duplicate_snippets, 1);
    assert_eq!(report.total_duplicate_occurrences, 3);
    assert_eq!(snippet.occurrence_count, 3);
    assert!(snippet.estimated_duplicate_tokens > 0);
    assert!(paths.contains(&"AGENTS.md"));
    assert!(paths.contains(&"rules/runtime.md"));
    assert!(paths.contains(&"skills/review/SKILL.md"));
    assert!(snippet.suggestion.contains("Keep one canonical copy"));

    let audit = collect_context_audit_report(&root, 10).expect("audit should load duplicates");
    let rendered = render_context_audit_report_with_width(&audit, 10, 100);
    assert_eq!(audit.static_duplicates.total_duplicate_snippets, 1);
    assert!(rendered.contains("Static Context Duplicates"));
    assert!(rendered.contains("Suggestion: Review and consolidate"));
    assert!(rendered.contains("does not edit files"));

    std::fs::remove_dir_all(root).expect("temp context root should be removed");
}

#[test]
fn context_static_duplicate_report_ignores_short_snippets_fences_and_backups() {
    let root = temp_context_root("static-dupes-ignored");
    std::fs::create_dir_all(root.join("rules")).expect("rules dir should be created");
    let duplicate = "This long paragraph is present only in a backup file, so the report must not count it as repeated static context that should be consolidated.";
    let fenced_duplicate = "This long fenced command sample repeats in two files but should not be reported because fenced content can be intentional examples.";
    std::fs::write(
        root.join("AGENTS.md"),
        format!("Keep terse.\n\n{duplicate}\n\n```text\n{fenced_duplicate}\n```\n"),
    )
    .expect("AGENTS should be written");
    std::fs::write(
        root.join("rules/team.md"),
        format!("Keep terse.\n\n```text\n{fenced_duplicate}\n```\n"),
    )
    .expect("rules file should be written");
    std::fs::write(
        root.join("rules/team.original.md"),
        format!("{duplicate}\n"),
    )
    .expect("backup should be written");

    let report =
        collect_context_static_duplicate_report(&root, 10).expect("duplicate report should load");

    assert_eq!(report.total_duplicate_snippets, 0);
    assert!(report.snippets.is_empty());
    assert!(report.suggestion.contains("No duplicate"));

    std::fs::remove_dir_all(root).expect("temp context root should be removed");
}

#[test]
fn context_blob_noise_detects_base64ish_blob() {
    let blob = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".repeat(4);
    let input = format!("header\n{blob}\nfooter\n");

    let report = detect_context_blob_noise(&input);

    assert!(report.is_noise());
    assert!(report.has_kind(ContextBlobNoiseKind::Base64Blob));
    assert_eq!(
        report
            .findings
            .iter()
            .find(|finding| finding.kind == ContextBlobNoiseKind::Base64Blob)
            .and_then(|finding| finding.line),
        Some(2)
    );
}

#[test]
fn context_blob_noise_detects_minified_json_and_javascript() {
    let json_entries = (0..48)
        .map(|index| {
            format!("\"pkg{index}\":{{\"version\":\"1.0.{index}\",\"deps\":[\"a\",\"b\"]}}")
        })
        .collect::<Vec<_>>()
        .join(",");
    let json = format!("{{{json_entries}}}");
    let js = (0..36)
        .map(|index| format!("!function(a){{return a+{index}}}({index});"))
        .collect::<String>();

    let json_report = detect_context_blob_noise(&json);
    let js_report = detect_context_blob_noise(&js);

    assert!(json_report.has_kind(ContextBlobNoiseKind::MinifiedJsJson));
    assert!(js_report.has_kind(ContextBlobNoiseKind::MinifiedJsJson));
}

#[test]
fn context_blob_noise_detects_lockfile_and_vendor_noise() {
    let cargo_lock = (0..5)
        .map(|index| {
            format!(
                "[[package]]\nname = \"dep{index}\"\nversion = \"0.1.{index}\"\nchecksum = \"abcdef{index}\"\n"
            )
        })
        .collect::<String>();
    let vendor_report = detect_context_blob_noise_for_path(
        std::path::Path::new("node_modules/example/index.js"),
        "module.exports = 1;\n",
    );

    let lock_report = detect_context_blob_noise(&cargo_lock);

    assert!(lock_report.has_kind(ContextBlobNoiseKind::LockfileOrVendor));
    assert!(vendor_report.has_kind(ContextBlobNoiseKind::LockfileOrVendor));
}

#[test]
fn context_blob_noise_detects_binaryish_text_without_nul_bytes() {
    let input = format!("plain text line\n{}payload\n", "\u{1}".repeat(6));

    let report = detect_context_blob_noise(&input);

    assert!(report.has_kind(ContextBlobNoiseKind::BinaryText));
}

#[test]
fn context_blob_noise_detects_repeated_path_flood() {
    let mut input = String::new();
    for index in 0..20 {
        input.push_str(&format!(
            "target/debug/build/prodex/out/generated.rs:{index}:1: generated path noise\n"
        ));
    }

    let report = detect_context_blob_noise(&input);

    assert!(report.has_kind(ContextBlobNoiseKind::RepeatedPathFlood));
}

#[test]
fn context_blob_noise_keeps_normal_diagnostic_text_clean() {
    let input = "\
error[E0308]: mismatched types
  --> crates/prodex-context/src/lib.rs:42:17
   |
42 |     let value: usize = \"nope\";
";

    let report = detect_context_blob_noise(input);

    assert!(!report.is_noise());
    assert!(!is_context_blob_noise(input));
}
