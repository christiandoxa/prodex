use super::*;

fn assert_no_critical_signal_loss(before: &str, after: &str) {
    let check = critical_signal_self_check(before, after);
    assert!(
        check.passed(),
        "lost critical signals: {:?}\ncompacted output:\n{}",
        check.lost,
        after
    );
}

fn test_cwd_prefix() -> String {
    std::env::current_dir()
        .expect("test cwd")
        .display()
        .to_string()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

fn temp_context_root(name: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-context-{name}-{}-{unique}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp context root should be created");
    root
}

#[test]
fn plain_command_output_strips_ansi_and_keeps_head_tail() {
    let input =
        "\u{1b}[31mline0\u{1b}[0m\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\n";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Plain,
        max_lines: 5,
        head_lines: 2,
        tail_lines: 2,
        max_line_chars: 80,
        ..CommandOutputCompactOptions::default()
    };

    let report = compact_command_output_with_options(input, &options);

    assert_eq!(report.detected_kind, CommandOutputKind::Plain);
    assert!(report.output.contains("line0"));
    assert!(report.output.contains("line1"));
    assert!(report.output.contains("line8"));
    assert!(report.output.contains("line9"));
    assert!(report.output.contains("[... omitted 6 lines ...]"));
    assert!(!report.output.contains("\u{1b}"));
}

#[test]
fn plain_command_output_uses_path_alias_for_repeated_absolute_cwd_prefix() {
    let cwd = test_cwd_prefix();
    let input = format!(
        "\
loaded {cwd}/src/main.rs
cached {cwd}/crates/prodex-context/src/lib.rs
"
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Plain,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Plain);
    assert!(
        report
            .output
            .contains(&format!("path aliases: $REPO={cwd}"))
    );
    assert!(report.output.contains("loaded $REPO/src/main.rs"));
    assert!(
        report
            .output
            .contains("cached $REPO/crates/prodex-context/src/lib.rs")
    );
}

#[test]
fn command_metadata_infers_output_kind_hints() {
    let cases = [
        (
            "{\"cmd\":\"cargo test -q\"}",
            CommandOutputKind::RustDiagnostics,
        ),
        (
            "command: cargo +nightly check --workspace",
            CommandOutputKind::RustDiagnostics,
        ),
        ("rg --json needle crates", CommandOutputKind::Search),
        ("grep -R needle src", CommandOutputKind::Search),
        ("git -C repo status --short", CommandOutputKind::GitStatus),
        ("git diff --stat", CommandOutputKind::GitDiff),
        ("git log --stat --oneline", CommandOutputKind::GitLog),
        ("pytest tests -q", CommandOutputKind::Diagnostics),
        ("python -m pytest tests", CommandOutputKind::Diagnostics),
        ("npx tsc --noEmit", CommandOutputKind::Diagnostics),
        ("npm test -- --runInBand", CommandOutputKind::Diagnostics),
        (
            "npm --prefix web run typecheck",
            CommandOutputKind::Diagnostics,
        ),
        ("ls -la crates", CommandOutputKind::FileList),
        (
            "find crates -maxdepth 2 -type f",
            CommandOutputKind::FileList,
        ),
        ("tree -L 2 crates", CommandOutputKind::FileList),
    ];

    for (metadata, expected) in cases {
        assert_eq!(
            infer_command_output_kind_from_metadata(metadata),
            Some(expected),
            "metadata: {metadata}"
        );
    }
}

#[test]
fn command_metadata_hint_compacts_single_search_match_as_search() {
    let input = "src/lib.rs:42:needle once\n";
    let hint = infer_command_output_kind_from_metadata("rg needle src");
    let report = compact_command_output_with_options_and_kind_hint(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
        hint,
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(
        report
            .output
            .contains("search summary: 1 matches across 1 files")
    );
    assert!(report.output.contains("src/lib.rs (1 matches):"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn command_metadata_hint_compacts_quiet_cargo_output_as_rust_diagnostics() {
    let input = "Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s\n";
    let hint = infer_command_output_kind_from_metadata("cargo check --workspace");
    let report = compact_command_output_with_options_and_kind_hint(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
        hint,
    );

    assert_eq!(report.detected_kind, CommandOutputKind::RustDiagnostics);
    assert!(report.output.contains("rust/cargo summary"));
    assert!(report.output.contains("Finished `dev` profile"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn command_metadata_hint_does_not_override_strong_output_detection() {
    let input = "\
src/app.ts:12:7 - error TS2322: Type 'string' is not assignable to type 'number'.

12 const count: number = value;
         ~~~~~
";
    let report = compact_command_output_with_options_and_kind_hint(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
        Some(CommandOutputKind::Search),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("error TS2322"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn git_status_short_output_groups_status_categories() {
    let input = "\
## main...origin/main
 M README.md
M  crates/prodex-context/src/lib.rs
R  old.txt -> new.txt
?? notes.txt
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitStatus);
    assert!(report.output.contains("branch: main...origin/main"));
    assert!(
        report
            .output
            .contains("staged (1): M crates/prodex-context/src/lib.rs")
    );
    assert!(report.output.contains("modified (1): M README.md"));
    assert!(report.output.contains("renamed (1): R old.txt -> new.txt"));
    assert!(report.output.contains("untracked (1): notes.txt"));
}

#[test]
fn git_status_output_shortens_repeated_absolute_cwd_prefix() {
    let cwd = test_cwd_prefix();
    let input = format!(
        "\
## main...origin/main
 M {cwd}/src/lib.rs
?? {cwd}/tests/new.rs
"
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitStatus);
    assert!(
        report
            .output
            .contains(&format!("path aliases: $REPO={cwd}"))
    );
    assert!(report.output.contains("modified (1): M $REPO/src/lib.rs"));
    assert!(report.output.contains("untracked (1): $REPO/tests/new.rs"));
}

#[test]
fn git_diff_output_keeps_summary_and_hunk_markers() {
    let input = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,12 @@
-old
+new0
+new1
+new2
+new3
+new4
+new5
+new6
+new7
+new8
+new9
 context
";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Auto,
        max_lines: 20,
        head_lines: 10,
        tail_lines: 3,
        max_line_chars: 120,
        ..CommandOutputCompactOptions::default()
    };

    let report = compact_command_output_with_options(input, &options);

    assert_eq!(report.detected_kind, CommandOutputKind::GitDiff);
    assert!(
        report
            .output
            .contains("git diff summary: 1 files, +10, -1, 1 hunks")
    );
    assert!(report.output.contains("src/lib.rs: +10, -1, 1 hunks"));
    assert!(report.output.contains("@@ -1,3 +1,12 @@"));
    assert!(report.output.contains("omitted"));
}

#[test]
fn search_output_groups_matches_by_file() {
    let input = "\
src/lib.rs:10:fn alpha() {}
src/lib.rs:20:5:let beta = true;
README.md:3:prodex context helper
";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Auto,
        max_search_matches_per_file: 1,
        ..CommandOutputCompactOptions::default()
    };

    let report = compact_command_output_with_options(input, &options);

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(
        report
            .output
            .contains("search summary: 3 matches across 2 files")
    );
    assert!(report.output.contains("README.md (1 matches):"));
    assert!(report.output.contains("src/lib.rs (2 matches):"));
    assert!(
        report
            .output
            .contains("[... 1 more matches in this file ...]")
    );
}

#[test]
fn search_output_shortens_repeated_absolute_cwd_prefix() {
    let cwd = test_cwd_prefix();
    let input = format!(
        "\
{cwd}/src/lib.rs:10:fn alpha() {{}}
{cwd}/src/lib.rs:20:fn beta() {{}}
{cwd}/README.md:3:prodex context helper
"
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_search_matches_per_file: 2,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(
        report
            .output
            .contains(&format!("path aliases: $REPO={cwd}"))
    );
    assert!(report.output.contains("$REPO/src/lib.rs (2 matches):"));
    assert!(report.output.contains("$REPO/README.md (1 matches):"));
}

#[test]
fn search_with_many_rust_file_matches_stays_search_output() {
    let input = "\
src/lib.rs:10:fn alpha() {}
src/lib.rs:20:fn beta() {}
src/app.rs:30:fn gamma() {}
crates/prodex-context/src/lib.rs:40:fn delta() {}
crates/prodex-context/tests/src/lib.rs:50:fn epsilon() {}
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
}

#[test]
fn file_list_output_summarizes_and_truncates_entries() {
    let input = "\
./src/main.rs
./src/lib.rs
./src/app_commands/context.rs
./crates/prodex-context/src/lib.rs
./crates/prodex-context/Cargo.toml
./README.md
./docs/testing.md
./target/debug/prodex
";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Auto,
        max_path_entries: 4,
        ..CommandOutputCompactOptions::default()
    };

    let report = compact_command_output_with_options(input, &options);

    assert_eq!(report.detected_kind, CommandOutputKind::FileList);
    assert!(report.output.contains("file list summary: 8 entries"));
    assert!(report.output.contains("top roots: src=3"));
    assert!(report.output.contains("extensions: rs=4"));
    assert!(
        report
            .output
            .contains("[... omitted 4 file-list entries ...]")
    );
}

#[test]
fn explicit_kind_overrides_auto_detection() {
    let input = "src/lib.rs:10:fn alpha() {}\n";
    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Plain,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.requested_kind, CommandOutputKind::Plain);
    assert_eq!(report.detected_kind, CommandOutputKind::Plain);
    assert_eq!(report.output, input);
}

#[test]
fn prompt_intent_extraction_finds_paths_symbols_tests_and_error_codes() {
    let prompt = "\
Fix `targetWidget` in crates/prodex-context/src/lib.rs and \
src/runtime_proxy/smart_context.rs:42. Failing test context::intent_extracts_paths \
plus test_should_compact_tool_output. Rust error E0277 and TS2322 mention \
ConfigLoader.load and renderWidget.
";

    let terms = extract_intent_terms_from_prompt(prompt);

    assert_eq!(
        terms,
        vec![
            "targetWidget",
            "crates/prodex-context/src/lib.rs",
            "src/runtime_proxy/smart_context.rs",
            "context::intent_extracts_paths",
            "test_should_compact_tool_output",
            "E0277",
            "TS2322",
            "ConfigLoader.load",
            "renderWidget",
        ]
    );
}

#[test]
fn prompt_intent_extraction_finds_quoted_identifiers_and_file_names() {
    let prompt = "Open 'README.md', `package.json`, \"handleSubmit\", and \
`tests::quota::keeps_affinity`; run \"cargo test smart_context::metadata_hint\"; \
ignore 'the' and `should`.";

    let terms = extract_intent_terms_from_prompt(prompt);

    assert_eq!(
        terms,
        vec![
            "README.md",
            "package.json",
            "handleSubmit",
            "tests::quota::keeps_affinity",
            "smart_context::metadata_hint",
        ]
    );
}

#[test]
fn prompt_intent_extraction_is_bounded_deduped_and_stable() {
    let mut prompt = "symbol_00 symbol_00 SYMBOL_00 ".to_string();
    for index in 1..40 {
        prompt.push_str(&format!("symbol_{index:02} "));
    }

    let terms = extract_intent_terms_from_prompt(&prompt);

    assert_eq!(terms.len(), MAX_EXTRACTED_INTENT_TERMS);
    assert_eq!(terms.first().map(String::as_str), Some("symbol_00"));
    assert_eq!(terms.last().map(String::as_str), Some("symbol_31"));
}

#[test]
fn prompt_intent_extraction_ignores_noisy_common_words() {
    let prompt = "Please update the smart context tool output compaction for common \
words and request prompt without adding noise. Ignore 'the' and `should`.";

    let terms = extract_intent_terms_from_prompt(prompt);

    assert!(terms.is_empty(), "unexpected terms: {terms:?}");
}

#[test]
fn extracted_prompt_intent_terms_drive_intent_compaction() {
    let prompt = "Fix targetWidget in src/target.ts after TS2322.";
    let terms = extract_intent_terms_from_prompt(prompt);
    let input = "\
src/alpha.ts(10,7): error TS2322: Type 'string' is not assignable to type 'number'.
10 const alpha: number = value;
src/target.ts(40,7): error TS2322: Type 'string' is not assignable to type 'number'.
40 const targetWidget: number = value;
Command failed with exit code 1
";

    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::Diagnostics,
                max_lines: 10,
                max_line_chars: 180,
                ..CommandOutputCompactOptions::default()
            },
            terms,
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("intent matches:"));
    assert!(report.output.contains("targetWidget"));
    assert!(report.output.contains("src/target.ts"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn intent_compaction_empty_terms_matches_existing_options_api() {
    let input = "\
line0
line1
line2
line3
line4
line5
line6
";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Plain,
        max_lines: 4,
        head_lines: 1,
        tail_lines: 1,
        max_line_chars: 80,
        ..CommandOutputCompactOptions::default()
    };

    let existing = compact_command_output_with_options(input, &options);
    let intent = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions {
            base: options,
            intent_terms: vec![String::new(), "   ".to_string()],
            kind_hint: None,
        },
    );

    assert_eq!(intent.detected_kind, existing.detected_kind);
    assert_eq!(intent.output, existing.output);
    assert_eq!(intent.compacted_lines, existing.compacted_lines);
    assert_eq!(
        intent.estimated_tokens_after,
        existing.estimated_tokens_after
    );
}

#[test]
fn intent_compaction_prioritizes_matching_diagnostic_lines() {
    let input = "\
src/alpha.ts(10,7): error TS2322: Type 'string' is not assignable to type 'number'.
10 const alpha: number = value;
src/beta.ts(20,7): error TS2322: Type 'string' is not assignable to type 'number'.
20 const beta: number = value;
src/gamma.ts(30,7): error TS2322: Type 'string' is not assignable to type 'number'.
30 const gamma: number = value;
src/target.ts(40,7): error TS2322: Type 'string' is not assignable to type 'number'.
40 const targetWidget: number = value;
Command failed with exit code 1
";
    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::Diagnostics,
                max_lines: 18,
                max_line_chars: 180,
                ..CommandOutputCompactOptions::default()
            },
            vec!["targetWidget".to_string()],
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("intent matches:"));
    assert!(report.output.contains("targetWidget"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn intent_compaction_prioritizes_matching_search_lines() {
    let input = "\
src/lib.rs:10:fn alpha() {}
src/lib.rs:20:fn target_symbol() {}
src/lib.rs:30:fn beta() {}
README.md:3:prodex context helper
";
    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::Search,
                max_lines: 12,
                max_search_matches_per_file: 1,
                max_line_chars: 160,
                ..CommandOutputCompactOptions::default()
            },
            vec!["target_symbol".to_string()],
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(report.output.contains("intent matches:"));
    assert!(report.output.contains("target_symbol"));
}

#[test]
fn intent_compaction_prioritizes_relevant_search_paths_and_summarizes_overflow() {
    let input = "\
src/alpha.rs:10:fn alpha() {}
src/beta.rs:20:fn beta() {}
crates/prodex-context/src/lib.rs:30:fn target_widget() {}
crates/prodex-context/src/lib.rs:40:let target_widget_enabled = true;
crates/prodex-context/tests/src/lib.rs:50:assert!(target_widget_enabled);
docs/context.md:3:target_widget docs
";

    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::Search,
                max_lines: 12,
                max_search_matches_per_file: 1,
                max_line_chars: 160,
                ..CommandOutputCompactOptions::default()
            },
            vec!["target_widget".to_string()],
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(report.output.contains("relevant search matches:"));
    assert!(report.output.contains("crates/prodex-context/src/lib.rs"));
    assert!(report.output.contains("fn target_widget()"));
    assert!(
        report
            .output
            .contains("search overflow: 2 other matches across 2 files")
    );
    assert!(report.output.contains("more relevant matches in this file"));
}

#[test]
fn intent_compaction_prioritizes_relevant_file_list_paths_and_summarizes_overflow() {
    let input = "\
src/main.rs
src/app.rs
crates/prodex-context/src/lib.rs
crates/prodex-context/tests/src/lib.rs
crates/prodex-cli/src/lib.rs
docs/context.md
README.md
";

    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::FileList,
                max_lines: 12,
                max_path_entries: 2,
                max_line_chars: 160,
                ..CommandOutputCompactOptions::default()
            },
            vec!["prodex-context".to_string()],
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::FileList);
    assert!(report.output.contains("intent matches: 2 paths"));
    assert!(report.output.contains("relevant paths:"));
    assert!(report.output.contains("crates/prodex-context/src/lib.rs"));
    assert!(
        report
            .output
            .contains("crates/prodex-context/tests/src/lib.rs")
    );
    assert!(
        report
            .output
            .contains("file-list overflow: 5 other entries")
    );
    assert!(report.output.contains("overflow roots:"));
}

#[test]
fn intent_compaction_prioritizes_matching_git_diff_lines() {
    let input = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,20 @@
 old
+new0
+new1
+new2
+new3
+new4
+new5
+new6
+new7
+new8
+pub fn target_symbol() {}
+new10
+new11
+new12
 context
";
    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::GitDiff,
                max_lines: 16,
                max_line_chars: 160,
                ..CommandOutputCompactOptions::default()
            },
            vec!["target_symbol".to_string()],
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitDiff);
    assert!(report.output.contains("intent matches:"));
    assert!(report.output.contains("+pub fn target_symbol() {}"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn intent_compaction_prioritizes_plain_lines_without_critical_signal_loss() {
    let input = "\
setup
line 1
line 2
error: hidden failure
src/main.rs:22:5
line 5
line 6
test tests::boom ... FAILED
line 8
line 9
interesting_symbol happened here
line 11
line 12
process didn't exit successfully: `cargo test` (exit status: 101)
tail
";
    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::Plain,
                max_lines: 12,
                head_lines: 2,
                tail_lines: 2,
                max_line_chars: 160,
                ..CommandOutputCompactOptions::default()
            },
            vec!["interesting_symbol".to_string()],
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Plain);
    assert!(report.output.contains("intent matches:"));
    assert!(report.output.contains("interesting_symbol happened here"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn rust_diagnostic_output_preserves_error_code_location_and_exit_status() {
    let input = "\
   Compiling dep_a v0.1.0
   Compiling dep_b v0.1.0
   Compiling prodex-context v0.1.0 (/repo/crates/prodex-context)
error[E0308]: mismatched types
  --> crates/prodex-context/src/lib.rs:42:17
   |
42 |     let value: usize = \"nope\";
   |                -----   ^^^^^^ expected `usize`, found `&str`
   |                |
   |                expected due to this
error: could not compile `prodex-context` (lib) due to 1 previous error
process didn't exit successfully: `rustc --crate-name prodex_context` (exit status: 101)
";
    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 50,
            max_line_chars: 180,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::RustDiagnostics);
    assert!(report.output.contains("error[E0308]: mismatched types"));
    assert!(
        report
            .output
            .contains("--> crates/prodex-context/src/lib.rs:42:17")
    );
    assert!(report.output.contains("exit status: 101"));
    assert!(report.output.contains("noise: compiling=3"));
}

#[test]
fn rust_test_output_preserves_failed_test_panic_and_backtrace_location() {
    let input = "\
running 4 tests
test context::keeps_success_noise_0 ... ok
test context::keeps_success_noise_1 ... ok
test context::critical_signal_failure ... FAILED
test context::keeps_success_noise_2 ... ok

failures:

---- context::critical_signal_failure stdout ----
thread 'context::critical_signal_failure' panicked at crates/prodex-context/tests/src/lib.rs:211:9:
assertion failed: left == right
stack backtrace:
   0: rust_begin_unwind
             at /rustc/library/std/src/panicking.rs:697:5
   1: core::panicking::panic_fmt
             at /rustc/library/core/src/panicking.rs:75:14

test result: FAILED. 3 passed; 1 failed; 0 ignored; finished in 0.01s
error: test failed, to rerun pass `-p prodex-context --lib`
";
    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 70,
            max_line_chars: 200,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::RustDiagnostics);
    assert!(report.output.contains("context::critical_signal_failure"));
    assert!(
        report
            .output
            .contains("crates/prodex-context/tests/src/lib.rs:211:9")
    );
    assert!(report.output.contains("stack backtrace:"));
    assert!(
        report
            .output
            .contains("/rustc/library/core/src/panicking.rs:75:14")
    );
    assert!(
        report
            .output
            .contains("test result: FAILED. 3 passed; 1 failed")
    );
    assert!(report.output.contains("passed_tests=3"));
}

#[test]
fn rust_success_noise_is_deduped_without_losing_final_result() {
    let input = "\
   Checking dep_a v0.1.0
   Checking dep_b v0.1.0
   Checking dep_c v0.1.0
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.23s
     Running unittests src/lib.rs (target/debug/deps/prodex_context-abc)
running 6 tests
test tests::alpha ... ok
test tests::beta ... ok
test tests::gamma ... ok
test tests::delta ... ok
test tests::epsilon ... ok
test tests::zeta ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; finished in 0.00s
";
    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 30,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::RustDiagnostics);
    assert!(report.output.contains("checking=3"));
    assert!(report.output.contains("passed_tests=6"));
    assert!(
        report
            .output
            .contains("test result: ok. 6 passed; 0 failed; 0 ignored")
    );
    assert!(!report.output.contains("test tests::alpha ... ok"));
}

#[test]
fn rust_short_clippy_output_preserves_lint_and_location() {
    let input = "\
    Checking prodex-context v0.1.0 (/repo/crates/prodex-context)
src/lib.rs:12:9: warning: used `unwrap()` on an `Option` value
   |
12 |     value.unwrap()
   |     ^^^^^^^^^^^^^^
   = note: `#[warn(clippy::unwrap_used)]` on by default
warning: `prodex-context` generated 1 warning
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 40,
            max_line_chars: 180,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::RustDiagnostics);
    assert!(
        report
            .output
            .contains("src/lib.rs:12:9: warning: used `unwrap()`")
    );
    assert!(report.output.contains("clippy::unwrap_used"));
    assert!(report.output.contains("warnings="));
}

#[test]
fn git_diff_stat_output_detects_git_diff_and_keeps_totals() {
    let input = "\
 src/lib.rs | 10 +++++-----
 README.md  |  2 ++
 2 files changed, 7 insertions(+), 5 deletions(-)
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitDiff);
    assert!(
        report
            .output
            .contains("git diff summary: stat-only, 2 file entries")
    );
    assert!(
        report
            .output
            .contains("stat totals: 2 files changed, 7 insertions(+), 5 deletions(-)")
    );
    assert!(report.output.contains("src/lib.rs | 10 +++++-----"));
}

#[test]
fn typescript_node_diagnostics_preserve_errors_locations_tests_and_exit_code() {
    let input = "\
> web@1.0.0 test /repo
> tsc --noEmit && jest --runInBand
src/index.ts(12,7): error TS2322: Type 'string' is not assignable to type 'number'.
src/service.ts:44:13 - error TS2345: Argument of type 'undefined' is not assignable.
FAIL tests/service.test.ts
  service
    rejects bad input

TypeError: Cannot read properties of undefined (reading 'id')
    at buildUser (/repo/src/service.ts:44:13)
    at Object.<anonymous> (/repo/tests/service.test.ts:9:5)

Test Suites: 1 failed, 1 total
Tests:       1 failed, 4 passed, 5 total
Command failed with exit code 1
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 90,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("error TS2322"));
    assert!(report.output.contains("src/index.ts(12,7)"));
    assert!(report.output.contains("src/service.ts:44:13"));
    assert!(report.output.contains("FAIL tests/service.test.ts"));
    assert!(report.output.contains("TypeError: Cannot read properties"));
    assert!(report.output.contains("Command failed with exit code 1"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn tight_diagnostics_budget_prioritizes_failure_before_success_noise() {
    let input = "\
> web@1.0.0 test /repo
PASS tests/alpha.test.ts
PASS tests/beta.test.ts
src/noisy.ts(1,1): warning TS6133: 'unused' is declared but its value is never read.
src/target.ts(40,7): error TS2322: Type 'string' is not assignable to type 'number'.
FAIL tests/target.test.ts
TypeError: Cannot read properties of undefined (reading 'id')
    at buildUser (/repo/src/target.ts:40:7)
Test Suites: 1 failed, 2 passed, 3 total
Tests:       1 failed, 8 passed, 9 total
Command failed with exit code 1
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 10,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("error TS2322"));
    assert!(report.output.contains("src/target.ts:40:7"));
    assert!(report.output.contains("FAIL tests/target.test.ts"));
    assert!(report.output.contains("Command failed with exit code 1"));
    assert!(!report.output.contains("PASS tests/alpha.test.ts"));
    assert!(!report.output.contains("warning TS6133"));
    assert!(report.compacted_lines <= 10);
}

#[test]
fn tight_rust_budget_prioritizes_failed_test_before_warnings_and_ok_tests() {
    let input = "\
warning: unused variable: `noise`
  --> crates/prodex-context/src/lib.rs:12:9
running 5 tests
test context::ok_0 ... ok
test context::ok_1 ... ok
test context::critical_failure ... FAILED
test context::ok_2 ... ok
test context::ok_3 ... ok

failures:

---- context::critical_failure stdout ----
thread 'context::critical_failure' panicked at crates/prodex-context/tests/src/lib.rs:311:9:
assertion failed: left == right
test result: FAILED. 4 passed; 1 failed; 0 ignored; finished in 0.01s
error: test failed, to rerun pass `-p prodex-context --lib`
process didn't exit successfully: `target/debug/deps/prodex_context` (exit status: 101)
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 12,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::RustDiagnostics);
    assert!(report.output.contains("context::critical_failure"));
    assert!(
        report
            .output
            .contains("crates/prodex-context/tests/src/lib.rs:311:9")
    );
    assert!(
        report
            .output
            .contains("test result: FAILED. 4 passed; 1 failed")
    );
    assert!(report.output.contains("exit status: 101"));
    assert!(!report.output.contains("test context::ok_0 ... ok"));
    assert!(!report.output.contains("warning: unused variable"));
    assert!(report.compacted_lines <= 12);
}

#[test]
fn diagnostics_compaction_shortens_repeated_absolute_repo_prefixes() {
    let input = "\
error: failed
  --> /home/doxa/IdeaProjects/prodex/crates/prodex-app/src/lib.rs:12:5
FAIL /home/doxa/IdeaProjects/prodex/tests/runtime_proxy.rs
Command failed with exit code 1
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Diagnostics,
            max_lines: 50,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(
        report
            .output
            .contains("path aliases: $REPO=/home/doxa/IdeaProjects/prodex")
    );
    assert!(
        report
            .output
            .contains("$REPO/crates/prodex-app/src/lib.rs:12:5")
    );
    assert!(report.output.contains("$REPO/tests/runtime_proxy.rs"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn diagnostics_output_shortens_repeated_absolute_cwd_prefix_without_signal_loss() {
    let cwd = test_cwd_prefix();
    let input = format!(
        "\
{cwd}/src/index.ts(12,7): error TS2322: Type 'string' is not assignable to type 'number'.
TypeError: Cannot read properties of undefined (reading 'id')
    at buildUser ({cwd}/src/service.ts:44:13)
    at Object.<anonymous> ({cwd}/tests/service.test.ts:9:5)
Command failed with exit code 1
"
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 80,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(
        report
            .output
            .contains(&format!("path aliases: $REPO={cwd}"))
    );
    assert!(report.output.contains("$REPO/src/index.ts(12,7)"));
    assert!(report.output.contains("$REPO/src/service.ts:44:13"));
    assert!(report.output.contains("$REPO/tests/service.test.ts:9:5"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn diagnostics_output_keeps_single_absolute_cwd_prefix() {
    let cwd = test_cwd_prefix();
    let input = format!(
        "\
{cwd}/src/index.ts(12,7): error TS2322: Type 'string' is not assignable to type 'number'.
Command failed with exit code 1
"
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 40,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains(&cwd));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn python_traceback_preserves_pytest_failure_exception_locations_and_exit_code() {
    let input = "\
============================= test session starts =============================
collected 3 items

tests/test_math.py::test_add PASSED
tests/test_math.py::test_divide FAILED
tests/test_api.py::test_timeout PASSED

================================== FAILURES ===================================
_______________________________ test_divide ________________________________
Traceback (most recent call last):
  File \"/repo/tests/test_math.py\", line 12, in test_divide
    divide(1, 0)
  File \"/repo/src/math_utils.py\", line 5, in divide
    return a / b
ZeroDivisionError: division by zero

FAILED tests/test_math.py::test_divide - ZeroDivisionError: division by zero
=========================== 1 failed, 2 passed in 0.12s ===========================
process finished with exit code 1
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 90,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("Traceback (most recent call last):"));
    assert!(
        report
            .output
            .contains("File \"$REPO/tests/test_math.py\", line 12")
    );
    assert!(
        report
            .output
            .contains("ZeroDivisionError: division by zero")
    );
    assert!(
        report
            .output
            .contains("FAILED tests/test_math.py::test_divide")
    );
    assert!(report.output.contains("exit code 1"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn generic_test_failure_preserves_failed_names_assertion_location_and_exit_status() {
    let input = "\
[runner] start
FAIL integration/login.spec
  case: rejects locked user
AssertionError: expected 403 but got 200
    at integration/login.spec:33:11
FAILED smoke::cli_can_report_status
Tests: 2 failed, 8 passed, 10 total
process exited with exit status 1
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 70,
            max_line_chars: 180,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("FAIL integration/login.spec"));
    assert!(report.output.contains("AssertionError: expected 403"));
    assert!(report.output.contains("integration/login.spec:33:11"));
    assert!(
        report
            .output
            .contains("FAILED smoke::cli_can_report_status")
    );
    assert!(report.output.contains("exit status 1"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn git_log_stat_output_summarizes_commits_and_stat_files() {
    let input = "\
commit aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
Author: Dev <dev@example.com>
Date:   Mon May 4 10:00:00 2026 +0700

    add diagnostics compaction

 crates/prodex-context/src/lib.rs       | 120 +++++++++++++++++++++++++
 crates/prodex-context/tests/src/lib.rs |  80 ++++++++++++++++
 2 files changed, 200 insertions(+)

commit bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
Author: Dev <dev@example.com>
Date:   Mon May 4 09:00:00 2026 +0700

    tune docs

 README.md | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 50,
            max_path_entries: 8,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitLog);
    assert!(
        report
            .output
            .contains("git log --stat summary: 2 commits, 3 stat file entries")
    );
    assert!(report.output.contains("commit: commit aaaaaaaaa"));
    assert!(
        report
            .output
            .contains("subject: add diagnostics compaction")
    );
    assert!(report.output.contains("2 files changed, 200 insertions(+)"));
    assert!(report.output.contains("README.md | 2 +-"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn noisy_success_output_summarizes_success_spam_without_losing_key_summary() {
    let input = "\
PASS tests/unit_0.test.ts
PASS tests/unit_1.test.ts
PASS tests/unit_2.test.ts
PASS tests/unit_3.test.ts
PASS tests/unit_4.test.ts
PASS tests/unit_5.test.ts
PASS tests/unit_6.test.ts
PASS tests/unit_7.test.ts
PASS tests/unit_8.test.ts
PASS tests/unit_9.test.ts
Test Suites: 10 passed, 10 total
Tests:       120 passed, 120 total
Snapshots:   0 total
Time:        4.12 s
Ran all test suites.
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::NoisySuccess);
    assert!(report.output.contains("success output summary"));
    assert!(report.output.contains("passed_suites=10"));
    assert!(report.output.contains("Test Suites: 10 passed, 10 total"));
    assert!(report.output.contains("Tests:       120 passed, 120 total"));
    assert!(!report.output.contains("PASS tests/unit_0.test.ts"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn noisy_success_output_detects_common_exit_zero_tool_noise() {
    let input = "\
ok  \tgithub.com/acme/prodex/pkg/a\t0.123s
ok  \tgithub.com/acme/prodex/pkg/b\t0.234s
?   \tgithub.com/acme/prodex/pkg/c\t[no test files]
============================= test session starts =============================
collected 12 items
............                                                             [100%]
============================== 12 passed in 1.23s ==============================
Test Files  4 passed (4)
Tests       12 passed (12)
Duration    1.42s
[INFO] --- maven-surefire-plugin:test (default-test) @ app ---
[INFO] Tests run: 12, Failures: 0, Errors: 0, Skipped: 0
[INFO] BUILD SUCCESS
> Task :test
BUILD SUCCESSFUL in 2s
4 actionable tasks: 4 executed
#1 [internal] load build definition from Dockerfile
#1 DONE 0.1s
Successfully built abcdef123456
All matched files use Prettier code style!
Running 12 tests using 4 workers
12 passed (8.2s)
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 40,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::NoisySuccess);
    assert!(report.output.contains("success output summary"));
    assert!(report.output.contains("go_test_ok=2"));
    assert!(report.output.contains("build_success="));
    assert!(report.output.contains("docker_summary="));
    assert!(report.output.contains("formatter_summary=1"));
    assert!(report.output.contains("12 passed (8.2s)"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn noisy_success_output_refuses_explicit_compaction_when_failure_signal_present() {
    let input = "\
PASS tests/unit_0.test.ts
PASS tests/unit_1.test.ts
FAIL tests/unit_2.test.ts
Tests: 1 failed, 2 passed, 3 total
Command failed with exit code 1
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::NoisySuccess,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::NoisySuccess);
    assert!(!report.output.contains("success output summary"));
    assert!(report.output.contains("FAIL tests/unit_2.test.ts"));
    assert!(report.output.contains("Command failed with exit code 1"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn successful_command_output_summary_compacts_long_install_build_and_list_output() {
    let mut input = String::new();
    input.push_str("added 82 packages, and audited 83 packages in 2s\n");
    input.push_str("found 0 vulnerabilities\n");
    input.push_str("vite v5.0.0 building for production...\n");
    for index in 0..30 {
        input.push_str(&format!("transforming src/module_{index}.ts\n"));
    }
    input.push_str("dist/index.html                  0.45 kB\n");
    input.push_str("dist/assets/app.js             24.12 kB\n");
    input.push_str("built in 1.42s\n");
    for index in 0..35 {
        input.push_str(&format!("src/generated/file_{index}.rs\n"));
    }

    let report = compact_successful_command_output_with_options(
        &input,
        &CommandSuccessOutputCompactOptions {
            command: Some("npm install && npm run build && find src -type f".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 20,
            max_touched_files: 8,
            max_key_lines: 8,
            max_line_chars: 160,
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert_eq!(report.critical_signals.total(), 0);
    assert!(report.touched_files >= 35);
    assert!(report.output.contains("successful command output"));
    assert!(
        report
            .output
            .contains("command: npm install && npm run build && find src -type f")
    );
    assert!(report.output.contains("exit: 0"));
    assert!(report.output.contains("counts:"));
    assert!(report.output.contains("noise:"));
    assert!(report.output.contains("touched_files="));
    assert!(report.output.contains("touched files ("));
    assert!(report.output.contains("src/generated/file_0.rs"));
    assert!(report.output.contains("[... "));
    assert!(!report.output.contains("src/generated/file_34.rs"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn successful_command_output_summary_compacts_common_tool_success_without_exit_code() {
    let mut input = String::new();
    for index in 0..24 {
        input.push_str(&format!(
            "ok  \tgithub.com/acme/prodex/pkg/{index}\t0.{index:03}s\n"
        ));
    }
    input.push_str("24 passed (9.2s)\n");

    let report = compact_successful_command_output_with_options(
        &input,
        &CommandSuccessOutputCompactOptions {
            command: Some("go test ./...".to_string()),
            exit_code: None,
            min_lines_to_compact: 8,
            max_touched_files: 8,
            max_key_lines: 6,
            max_line_chars: 160,
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert!(report.output.contains("command: go test ./..."));
    assert!(report.output.contains("noise: go_test_ok=24"));
    assert!(report.output.contains("24 passed (9.2s)"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn successful_command_output_summary_refuses_suspected_failure_even_with_exit_zero() {
    let input = "\
[INFO] BUILD FAILURE
Tests run: 12, Failures: 1, Errors: 0, Skipped: 0
1 failed, 11 passed in 1.23s
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("mvn test".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(!report.compacted);
    assert!(report.failure_suspected);
    assert_eq!(report.output, input);
}

#[test]
fn successful_command_output_summary_preserves_failure_critical_lines_exactly() {
    let input = "\
Installing dependencies
Downloaded package alpha
error: build script failed
src/build.rs:42:9
process didn't exit successfully: `cargo build` (exit status: 101)
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("cargo build".to_string()),
            exit_code: Some(101),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(!report.compacted);
    assert!(report.failure_suspected);
    assert!(report.output.contains("error: build script failed"));
    assert!(report.output.contains("src/build.rs:42:9"));
    assert!(
        report
            .output
            .contains("process didn't exit successfully: `cargo build` (exit status: 101)")
    );
    assert_eq!(report.output, input);
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn successful_command_output_summary_is_conservative_when_critical_signal_present() {
    let mut input = String::new();
    for index in 0..30 {
        input.push_str(&format!("PASS tests/unit_{index}.test.ts\n"));
    }
    input.push_str("warning: generated bindings changed\n");
    input.push_str("crates/prodex-context/src/lib.rs:123:5\n");
    input.push_str("Test Suites: 30 passed, 30 total\n");

    let report = compact_successful_command_output_with_options(
        &input,
        &CommandSuccessOutputCompactOptions {
            command: Some("npm test".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(!report.compacted);
    assert!(report.failure_suspected);
    assert!(
        report
            .output
            .contains("warning: generated bindings changed")
    );
    assert!(
        report
            .output
            .contains("crates/prodex-context/src/lib.rs:123:5")
    );
    assert!(report.output.contains("PASS tests/unit_0.test.ts"));
    assert_no_critical_signal_loss(&input, &report.output);
}

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
    assert!(
        report
            .output
            .contains("search summary: 2 matches across 2 files")
    );
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
-rw-r--r-- 1 doxa doxa 10 May 1 12:00 Cargo.toml
-rw-r--r-- 1 doxa doxa 20 May 1 12:00 README.md
drwxr-xr-x 2 doxa doxa 4096 May 1 12:00 crates
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
    assert!(report.output.contains("file list summary: 4 entries"));
    assert!(report.output.contains("Cargo.toml"));
    assert!(report.output.contains("README.md"));
    assert!(report.output.contains("src/main.rs"));
}

#[test]
fn json_logish_plain_output_keeps_middle_error_line() {
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

    assert_eq!(report.detected_kind, CommandOutputKind::Plain);
    assert!(report.output.contains("critical lines:"));
    assert!(report.output.contains("\"level\":\"error\""));
    assert!(report.output.contains("database unavailable"));
    assert!(report.output.contains("src/db.rs:44:9"));
    assert!(report.compacted_lines <= 12);
}

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
