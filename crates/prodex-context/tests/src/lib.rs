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

fn assert_token_regression_budget(
    before: &str,
    report: &CommandOutputCompactReport,
    min_saved_percent: usize,
) {
    assert_no_critical_signal_loss(before, &report.output);
    assert!(
        report.estimated_tokens_after < report.estimated_tokens_before,
        "compaction did not save tokens: before={}, after={}\n{}",
        report.estimated_tokens_before,
        report.estimated_tokens_after,
        report.output
    );
    let saved = report
        .estimated_tokens_before
        .saturating_sub(report.estimated_tokens_after);
    assert!(
        saved.saturating_mul(100)
            >= report
                .estimated_tokens_before
                .saturating_mul(min_saved_percent),
        "saved {saved}/{} tokens, expected at least {min_saved_percent}%\n{}",
        report.estimated_tokens_before,
        report.output
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

#[path = "lib/basics.rs"]
mod basics;

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
            .contains("sum: git diff files=1, +10, -1, hunks=1")
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
    assert!(report.output.contains("sum: search matches=3, files=2"));
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
    assert!(report.output.contains("sum: files entries=8"));
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
    assert!(report.output.contains("int:"));
    assert!(report.output.contains("targetWidget"));
    assert!(report.output.contains("src/target.ts"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn full_prompt_intent_terms_are_extracted_before_compaction() {
    let prompt = "Fix targetWidget in src/target.ts after TS2322; ignore src/noise.ts.";
    let input = "\
src/noise.ts(10,7): error TS2322: Type 'string' is not assignable to type 'number'.
10 const noise: number = value;
src/target.ts(40,7): error TS2322: Type 'string' is not assignable to type 'number'.
40 const targetWidget: number = value;
Command failed with exit code 1
";

    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::Diagnostics,
                max_lines: 9,
                max_line_chars: 180,
                ..CommandOutputCompactOptions::default()
            },
            vec![prompt.to_string()],
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
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
    assert!(report.output.contains("int:"));
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
    assert!(report.output.contains("int:"));
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
    assert!(report.output.contains("rel search:"));
    assert!(report.output.contains("crates/prodex-context/src/lib.rs"));
    assert!(report.output.contains("fn target_widget()"));
    assert!(
        report
            .output
            .contains("overflow: 2 other matches across 2 files")
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
    assert!(report.output.contains("int: 2 paths"));
    assert!(report.output.contains("rel paths:"));
    assert!(report.output.contains("crates/prodex-context/src/lib.rs"));
    assert!(
        report
            .output
            .contains("crates/prodex-context/tests/src/lib.rs")
    );
    assert!(report.output.contains("overflow: 5 other file entries"));
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
    assert!(report.output.contains("int:"));
    assert!(report.output.contains("+pub fn target_symbol() {}"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn git_diff_compaction_keeps_semantic_hunk_context_and_reduces_noise() {
    let mut input = String::from(
        "\
diff --git a/src/app.rs b/src/app.rs
index 1111111..2222222 100644
--- a/src/app.rs
+++ b/src/app.rs
@@ -1,4 +1,28 @@ fn target_handler()
 pub fn target_handler() {
",
    );
    for index in 0..24 {
        input.push_str(&format!("+    let noise_{index} = {index};\n"));
    }
    input.push_str(" }\n");

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::GitDiff,
            max_lines: 18,
            max_line_chars: 180,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitDiff);
    assert!(report.output.contains("ctx=fn target_handler"));
    assert!(report.output.contains("pub fn target_handler()"));
    assert!(report.output.contains("omitted"));
    assert!(!report.output.contains("noise_23"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn intent_git_diff_compaction_keeps_adjacent_context() {
    let input = "\
diff --git a/src/app.rs b/src/app.rs
index 1111111..2222222 100644
--- a/src/app.rs
+++ b/src/app.rs
@@ -1,8 +1,10 @@ fn unrelated()
+let before_target = prepare();
+let targetWidget = build_target();
+let after_target = verify();
+let unrelated_noise = 1;
";

    let report = compact_command_output_with_intent_options(
        input,
        &CommandOutputIntentCompactOptions::new(
            CommandOutputCompactOptions {
                kind: CommandOutputKind::GitDiff,
                max_lines: 12,
                max_line_chars: 180,
                ..CommandOutputCompactOptions::default()
            },
            vec!["targetWidget".to_string()],
        ),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitDiff);
    assert!(report.output.contains("int: git diff focus"));
    assert!(report.output.contains("before_target"));
    assert!(report.output.contains("targetWidget"));
    assert!(report.output.contains("after_target"));
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
    assert!(report.output.contains("int:"));
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
    assert!(report.output.contains("sum: git diff stat_only entries=2"));
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
fn diagnostics_compaction_promotes_root_cause_and_collapses_pass_noise() {
    let mut input = String::from(
        "\
> app@1.0.0 test /repo
",
    );
    for index in 0..40 {
        input.push_str(&format!("PASS tests/noise_{index}.test.ts\n"));
    }
    input.push_str(
        "\
FAIL tests/target.test.ts
AssertionError: expected 400 but got 200
    at submitOrder (/repo/src/order.ts:88:13)
Test Suites: 1 failed, 40 passed, 41 total
Tests:       1 failed, 120 passed, 121 total
Command failed with exit code 1
",
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 44,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("root causes"));
    assert!(report.output.contains("AssertionError: expected 400"));
    assert!(report.output.contains("tests/target.test.ts"));
    assert!(report.output.contains("src/order.ts:88:13"));
    assert!(report.output.contains("passed_suites=40"));
    assert!(!report.output.contains("PASS tests/noise_39.test.ts"));
    assert_no_critical_signal_loss(&input, &report.output);
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
  --> /workspace/prodex/crates/prodex-app/src/lib.rs:12:5
FAIL /workspace/prodex/tests/runtime_proxy.rs
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
            .contains("path aliases: $REPO=/workspace/prodex")
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
            .contains("sum: git log --stat commits=2, stat_files=3")
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
    assert!(report.output.contains("sum: success"));
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
    assert!(report.output.contains("sum: success"));
    assert!(report.output.contains("go_test_ok=2"));
    assert!(report.output.contains("build_success="));
    assert!(report.output.contains("docker_summary="));
    assert!(report.output.contains("formatter_summary=1"));
    assert!(report.output.contains("12 passed (8.2s)"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn noisy_success_output_detects_modern_exit_zero_tool_noise() {
    let input = "\
All checks passed!
Success: no issues found in 42 source files
Resolved 42 packages in 10ms
Prepared 3 packages in 20ms
Installed 3 packages in 30ms
Requirement already satisfied: anyio in .venv/lib/python3.12/site-packages
Successfully installed httpx-0.28.1
INFO: Analyzed target //app:bin (1 packages loaded, 1 target configured).
Target //app:bin up-to-date:
INFO: Build completed successfully, 1 total action
NX Successfully ran target build for project web
Tasks: 3 successful, 3 total
PASS [   0.100s] prodex::smoke smoke_test
Summary [   0.200s] 1 test run: 1 passed
[+] Running 2/2
Container prodex-db-1 Started
Network prodex_default Created
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 80,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::NoisySuccess);
    assert!(report.output.contains("sum: success"));
    assert!(report.output.contains("formatter_summary=1"));
    assert!(report.output.contains("typecheck_summary=1"));
    assert!(report.output.contains("python_packages=5"));
    assert!(report.output.contains("bazel_summary=2"));
    assert!(report.output.contains("nx_summary=1"));
    assert!(report.output.contains("turbo_summary=1"));
    assert!(report.output.contains("nextest_summary=1"));
    assert!(report.output.contains("docker_compose=3"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn noisy_success_output_compacts_coverage_playwright_cypress_and_junit_success() {
    let input = "\
-------------------|---------|----------|---------|---------|-------------------
File               | % Stmts | % Branch | % Funcs | % Lines | Uncovered Line #s
-------------------|---------|----------|---------|---------|-------------------
All files          |   92.31 |    88.00 |   90.00 |   92.31 |
src/app.ts         |   93.10 |    87.50 |   90.00 |   93.10 | 44
src/api.ts         |   91.20 |    88.50 |   90.00 |   91.20 | 18
Running 12 tests using 4 workers
12 passed (8.2s)
✔ All specs passed!
Spec                                              Tests  Passing  Failing  Pending  Skipped
tests/app.cy.ts                                      4        4        0        0        0
<testsuite name=\"unit\" tests=\"12\" failures=\"0\" errors=\"0\" skipped=\"0\"/>
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
    assert!(report.output.contains("sum: success"));
    assert!(report.output.contains("coverage="));
    assert!(report.output.contains("playwright="));
    assert!(report.output.contains("cypress="));
    assert!(report.output.contains("junit_xml=1"));
    assert!(!report.output.contains("src/api.ts"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn diagnostics_preserve_junit_eslint_and_typescript_failures() {
    let input = "\
<testsuite name=\"unit\" tests=\"2\" failures=\"1\" errors=\"0\">
  <testcase classname=\"app\" name=\"rejects bad input\">
    <failure message=\"expected 400\">src/api.test.ts:18:7 assertion failed</failure>
  </testcase>
</testsuite>
/repo/src/app.ts
  12:5  error  Unexpected any.  @typescript-eslint/no-explicit-any
src/index.ts(9,3): error TS2322: Type 'string' is not assignable to type 'number'.
Command failed with exit code 1
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 80,
            max_line_chars: 220,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("<failure message=\"expected 400\">"));
    assert!(report.output.contains("src/api.test.ts:18:7"));
    assert!(report.output.contains("@typescript-eslint/no-explicit-any"));
    assert!(report.output.contains("src/index.ts(9,3): error TS2322"));
    assert!(report.output.contains("exit code 1"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn command_metadata_hints_install_update_and_docker_success_as_noisy_success() {
    for command in [
        "cargo update",
        "npm install",
        "pnpm install --frozen-lockfile",
        "yarn install",
        "bun install",
        "docker build -t app .",
        "docker buildx build .",
        "gradle test",
        "./gradlew test",
        "mvn test",
        "./mvnw test",
        "bazelisk test //...",
        "uv sync",
    ] {
        assert_eq!(
            command_output_kind_hint_for_command(command),
            Some(CommandOutputKind::NoisySuccess),
            "{command}"
        );
    }
}

#[test]
fn successful_command_output_summary_compacts_install_build_and_ci_success_noise() {
    let input = "\
Updating crates.io index
Locking 4 packages to latest compatible versions
Downloading crates ...
Downloaded serde v1.0.0
added 120 packages, and audited 121 packages in 4s
found 0 vulnerabilities
Lockfile is up to date
Packages: +42
Progress: resolved 42, reused 40, downloaded 2, added 42
#1 [internal] load build definition from Dockerfile
#1 DONE 0.1s
#2 [internal] load metadata for docker.io/library/debian:bookworm
#2 DONE 0.2s
writing image sha256:abc123
naming to docker.io/library/app:latest
Resolved 42 packages in 10ms
Prepared 3 packages in 20ms
Installed 3 packages in 30ms
Test Suites: 8 passed, 8 total
Tests:       64 passed, 64 total
found 0 errors
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some(
                "cargo update && npm install && pnpm install && docker build . && uv sync"
                    .to_string(),
            ),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert!(report.output.contains("pcs: success cmd"));
    assert!(report.output.contains("docker_summary="));
    assert!(report.output.contains("found 0 errors"));
    assert!(!report.output.contains("Downloading crates ..."));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn successful_command_output_summary_keeps_nonzero_ci_failure_counts_exact() {
    let input = "\
Test Suites: 7 passed, 1 failed, 8 total
Tests:       64 passed, 2 failed, 66 total
found 2 errors
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("npm test".to_string()),
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
fn successful_command_output_summary_refuses_modern_tool_failure_signals() {
    let input = "\
Found 2 errors.
src/app.py:10:1: F401 unused import
src/api.py:22:5: error: incompatible type
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("ruff check && mypy src".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(!report.compacted);
    assert!(report.failure_suspected);
    assert_eq!(report.output, input);
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
    assert!(!report.output.contains("sum: success"));
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
    assert!(report.output.contains("success cmd"));
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
fn successful_command_output_summary_omits_touched_files_for_pure_success_noise() {
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
        input.push_str(&format!("emitted src/generated/file_{index}.rs\n"));
    }
    input.push_str("Test Suites: 4 passed, 4 total\n");
    input.push_str("Tests:       20 passed, 0 failed, 20 total\n");
    input.push_str("found 0 errors\n");

    let report = compact_successful_command_output_with_options(
        &input,
        &CommandSuccessOutputCompactOptions {
            command: Some("npm install && npm run build && npm test".to_string()),
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
    assert!(report.compacted_lines < 14);
    assert!(report.output.contains("success cmd"));
    assert!(report.output.contains("counts:"));
    assert!(report.output.contains("noise:"));
    assert!(
        report
            .output
            .contains("Tests:       20 passed, 0 failed, 20 total")
    );
    assert!(report.output.contains("found 0 errors"));
    assert!(!report.output.contains("top roots:"));
    assert!(!report.output.contains("extensions:"));
    assert!(!report.output.contains("touched files ("));
    assert!(!report.output.contains("src/generated/file_0.rs"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn successful_command_output_summary_uses_short_form_for_pure_noise() {
    let mut input = String::new();
    for index in 0..40 {
        input.push_str(&format!(
            "Progress: resolved {}, reused {}, downloaded 0, added 0\n",
            index + 1,
            index
        ));
    }

    let report = compact_successful_command_output_with_options(
        &input,
        &CommandSuccessOutputCompactOptions {
            command: Some("pnpm install".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 10,
            max_line_chars: 160,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert_eq!(report.critical_signals.total(), 0);
    assert!(report.compacted_lines <= 3);
    assert!(report.output.contains("pcs: ok lines=40"));
    assert!(report.output.contains("noise:"));
    assert!(report.output.contains("cmd: pnpm install"));
    assert!(!report.output.contains("counts:"));
    assert!(!report.output.contains("Progress: resolved 40"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn successful_command_output_summary_compacts_requested_tool_success_noise() {
    let cases = [
        (
            "gradle test",
            "\
> Task :compileJava UP-TO-DATE
> Task :test
Gradle Test Executor 1 > AppTest > works() PASSED
3 tests completed
BUILD SUCCESSFUL in 2s
4 actionable tasks: 2 executed, 2 up-to-date
",
            ["gradle_test=", "build_success=", "gradle_tasks="].as_slice(),
        ),
        (
            "./gradlew test",
            "\
> Task :test
Test run finished after 123 ms
[         2 tests successful      ]
BUILD SUCCESSFUL in 1s
1 actionable task: 1 executed
",
            ["gradle_test=", "build_success=", "gradle_tasks="].as_slice(),
        ),
        (
            "mvn test",
            "\
[INFO] Scanning for projects...
[INFO] --- maven-surefire-plugin:3.2.5:test (default-test) @ app ---
[INFO] -------------------------------------------------------
[INFO] T E S T S
[INFO] Running com.example.AppTest
[INFO] Tests run: 2, Failures: 0, Errors: 0, Skipped: 0
[INFO] Results:
[INFO] BUILD SUCCESS
[INFO] Total time:  1.234 s
[INFO] Finished at: 2026-05-05T00:00:00Z
",
            ["maven_test=", "maven_summary=", "build_success="].as_slice(),
        ),
        (
            "./mvnw test",
            "\
[INFO] Running com.example.OtherTest
[INFO] Tests run: 1, Failures: 0, Errors: 0, Skipped: 0
[INFO] BUILD SUCCESS
[INFO] Total time:  0.987 s
[INFO] Finished at: 2026-05-05T00:00:00Z
",
            ["maven_test=", "maven_summary=", "build_success="].as_slice(),
        ),
        (
            "pytest --cov=src",
            "\
..                                                                       [100%]
2 passed in 0.12s
---------- coverage: platform linux, python 3.12.0-final-0 -----------
Name         Stmts   Miss  Cover   Missing
src/app.py      12      0   100%
src/api.py      20      1    95%   44
TOTAL           32      1    97%
Required test coverage of 80% reached. Total coverage: 96.88%
Coverage XML written to file coverage.xml
",
            ["pytest_progress=", "passed_tests=", "coverage="].as_slice(),
        ),
        (
            "npm install",
            "\
added 120 packages, and audited 121 packages in 4s
10 packages are looking for funding
run `npm fund` for details
found 0 vulnerabilities
",
            ["packages_added=", "vulnerability_summary="].as_slice(),
        ),
        (
            "pnpm install",
            "\
Scope: all 3 workspace projects
Lockfile is up to date, resolution step is skipped
Already up to date
Progress: resolved 42, reused 42, downloaded 0, added 0
Done in 1.2s using pnpm v9.0.0
",
            [
                "package_install=",
                "packages_up_to_date=",
                "package_progress=",
            ]
            .as_slice(),
        ),
        (
            "yarn install",
            "\
yarn install v1.22.22
[1/4] Resolving packages...
[2/4] Fetching packages...
[3/4] Linking dependencies...
[4/4] Building fresh packages...
success Saved lockfile.
Done in 3.21s.
",
            ["package_install=", "done="].as_slice(),
        ),
        (
            "bun install",
            "\
bun install v1.1.0
Resolved, downloaded and extracted [10]
Saved lockfile
10 packages installed [50.00ms]
",
            ["package_install="].as_slice(),
        ),
        (
            "docker buildx build .",
            "\
#0 building with \"default\" instance using docker-container driver
#1 [internal] load build definition from Dockerfile
#1 transferring dockerfile: 2.1kB done
#1 DONE 0.0s
#2 exporting to image
#2 exporting layers done
#2 writing image sha256:abc123 done
#2 naming to docker.io/library/app:latest done
",
            ["docker_buildx=", "docker_steps="].as_slice(),
        ),
        (
            "bazel test //...",
            "\
INFO: Analyzed 2 targets (0 packages loaded, 0 targets configured).
INFO: Found 2 test targets...
//src/app:app_test                                                   PASSED in 0.1s
//src/api:api_test                                                   PASSED in 0.2s
INFO: Elapsed time: 0.456s, Critical Path: 0.12s
INFO: 2 processes: 2 linux-sandbox.
INFO: Build completed successfully, 2 total actions
Executed 2 out of 2 tests: 2 tests pass.
",
            ["bazel_test=", "bazel_steps=", "bazel_summary="].as_slice(),
        ),
        (
            "bazelisk test //...",
            "\
INFO: Found 1 test target...
//src/app:app_test                                                   PASSED in 0.1s
INFO: Build completed successfully, 1 total action
Executed 1 out of 1 test: 1 test passes.
",
            ["bazel_test=", "bazel_summary="].as_slice(),
        ),
    ];

    for (command, input, expected_labels) in cases {
        let report = compact_successful_command_output_with_options(
            input,
            &CommandSuccessOutputCompactOptions {
                command: Some(command.to_string()),
                exit_code: Some(0),
                min_lines_to_compact: 1,
                max_touched_files: 8,
                max_key_lines: 8,
                max_line_chars: 180,
            },
        );

        assert!(report.compacted, "{command}: {}", report.output);
        assert!(!report.failure_suspected, "{command}");
        assert_eq!(report.critical_signals.total(), 0, "{command}");
        assert!(
            report.output.contains("noise:"),
            "{command}: {}",
            report.output
        );
        for label in expected_labels {
            assert!(
                report.output.contains(label),
                "{command}: missing {label}\n{}",
                report.output
            );
        }
        assert_no_critical_signal_loss(input, &report.output);
    }
}

#[test]
fn successful_command_output_summary_refuses_requested_tool_warnings_failures_and_nonzero() {
    let cases = [
        (
            "gradle test",
            "\
> Task :test FAILED
Execution failed for task ':test'.
BUILD FAILED in 1s
",
            Some(0),
        ),
        (
            "mvn test",
            "\
[ERROR] Tests run: 2, Failures: 1, Errors: 0, Skipped: 0
[INFO] BUILD FAILURE
",
            Some(0),
        ),
        (
            "pytest --cov=src",
            "\
Traceback (most recent call last):
  File \"tests/test_app.py\", line 7, in test_app
AssertionError: boom
Required test coverage of 80% not reached. Total coverage: 72.00%
",
            Some(0),
        ),
        (
            "npm install",
            "\
npm WARN deprecated left-pad@1.3.0: use String.prototype.padStart()
added 1 package, and audited 2 packages in 1s
",
            Some(0),
        ),
        (
            "pnpm install",
            "\
ERR_PNPM_OUTDATED_LOCKFILE Cannot install with frozen-lockfile
",
            Some(0),
        ),
        (
            "yarn install",
            "\
yarn warning package.json: No license field
Done in 0.10s.
",
            Some(0),
        ),
        (
            "bun install",
            "\
bun warning package.json: missing license
10 packages installed [50.00ms]
",
            Some(0),
        ),
        (
            "docker buildx build .",
            "\
#6 ERROR: process \"/bin/sh -c false\" did not complete successfully: exit code: 1
ERROR: failed to solve: process did not complete successfully
",
            Some(0),
        ),
        (
            "bazel test //...",
            "\
//src/app:app_test                                                   FAILED in 0.1s
Executed 2 out of 2 tests: 1 test passes and 1 fails locally.
",
            Some(0),
        ),
        (
            "npm install",
            "\
added 120 packages, and audited 121 packages in 4s
found 0 vulnerabilities
",
            Some(1),
        ),
    ];

    for (command, input, exit_code) in cases {
        let report = compact_successful_command_output_with_options(
            input,
            &CommandSuccessOutputCompactOptions {
                command: Some(command.to_string()),
                exit_code,
                min_lines_to_compact: 1,
                ..CommandSuccessOutputCompactOptions::default()
            },
        );

        assert!(!report.compacted, "{command}: {}", report.output);
        assert!(report.failure_suspected, "{command}");
        assert_eq!(report.output, input, "{command}");
        assert_no_critical_signal_loss(input, &report.output);
    }
}

#[test]
fn successful_command_output_summary_uses_short_form_for_tsc_success() {
    let input = "\
Projects in this build:
    * tsconfig.json
Building project 'tsconfig.json'...
Project 'tsconfig.json' is up to date because newest input is older than output
Updating unchanged output timestamps of project 'tsconfig.json'...
Projects in this build:
    * packages/app/tsconfig.json
Project 'packages/app/tsconfig.json' is up to date
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("npx tsc --build --pretty false".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            max_line_chars: 160,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert!(report.output.contains("pcs: ok"));
    assert!(report.output.contains("typecheck_summary="));
    assert!(!report.output.contains("packages/app/tsconfig.json"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn successful_command_output_summary_uses_short_form_for_next_build_success() {
    let input = "\
Next.js 15.3.1
Creating an optimized production build ...
Compiled successfully
Linting and checking validity of types
Collecting page data
Generating static pages (4/4)
Finalizing page optimization
Collecting build traces
+ First Load JS shared by all 87.1 kB
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("next build --profile".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            max_line_chars: 160,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert!(report.output.contains("pcs: ok"));
    assert!(report.output.contains("next="));
    assert!(
        !report
            .output
            .contains("Creating an optimized production build")
    );
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn successful_command_output_summary_uses_short_form_for_cargo_doc_success_with_paths() {
    let input = "\
Documenting prodex v0.77.0 (/workspace/prodex)
Documenting prodex-context v0.77.0 (/workspace/prodex/crates/prodex-context)
Generated /workspace/prodex/target/doc/prodex/index.html
Generated /workspace/prodex/target/doc/prodex_context/index.html
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.14s
Documenting prodex-cli v0.77.0 (/workspace/prodex/crates/prodex-cli)
Generated /workspace/prodex/target/doc/prodex_cli/index.html
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.32s
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("cargo doc --workspace --no-deps".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            max_line_chars: 160,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert!(report.output.contains("pcs: ok"));
    assert!(report.output.contains("documenting="));
    assert!(!report.output.contains("target/doc/prodex_cli/index.html"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn successful_command_output_summary_refuses_warning_only_build_output() {
    let input = "\
Next.js 15.3.1
Creating an optimized production build ...
Compiled with warnings
warning TS6133: 'unused' is declared but its value is never read.
Linting and checking validity of types
Collecting page data
Generating static pages (4/4)
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("next build".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(!report.compacted);
    assert!(report.failure_suspected);
    assert_eq!(report.output, input);
    assert_no_critical_signal_loss(input, &report.output);
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
fn successful_command_output_summary_compacts_format_lint_quiet_and_verbose_test_success_noise() {
    let cases = [
        (
            "cargo clippy --fix --allow-dirty",
            "\
    Checking prodex-context v0.1.0
     Fixing src/lib.rs
      Fixed crates/prodex-context/tests/src/lib.rs (1 fix)
    Finished `dev` profile [unoptimized] target(s) in 1.23s
",
            ["checking=", "cargo_fix=", "finished="].as_slice(),
            "Fixing src/lib.rs",
        ),
        (
            "cargo fmt --all",
            "\
Formatting src/lib.rs
Formatting crates/prodex-context/src/lib.rs
Formatting crates/prodex-context/tests/src/lib.rs
",
            ["formatting=3"].as_slice(),
            "Formatting src/lib.rs",
        ),
        (
            "biome check --write .",
            "\
Checked 12 files in 10ms. No fixes applied.
Formatted 2 files in 3ms. Fixed 1 file.
No fixes applied.
",
            ["biome_summary=3"].as_slice(),
            "Checked 12 files",
        ),
        (
            "oxlint --fix",
            "\
Found 0 warnings and 0 errors.
Finished in 12ms on 42 files with 0 warnings.
",
            ["typecheck_summary=1", "oxlint_summary=1"].as_slice(),
            "Finished in 12ms",
        ),
        (
            "pytest -q",
            "\
.......                                                                  [100%]
7 passed, 2 skipped in 0.12s
",
            ["pytest_progress=1", "passed_tests=1"].as_slice(),
            ".......",
        ),
        (
            "go test -v ./...",
            "\
=== RUN   TestAlpha
=== PAUSE TestAlpha
=== CONT  TestAlpha
--- PASS: TestAlpha (0.00s)
=== RUN   TestBeta
--- PASS: TestBeta (0.01s)
PASS
ok  \tgithub.com/acme/prodex/pkg/foo\t0.123s
",
            [
                "go_test_run=2",
                "go_test_pause=1",
                "go_test_cont=1",
                "go_test_pass=2",
                "go_test_pass_summary=1",
                "go_test_ok=1",
            ]
            .as_slice(),
            "=== RUN   TestAlpha",
        ),
        (
            "pnpm test --reporter=dot",
            "\
............................................................

60 passed in 1.23s
",
            ["dot_progress=1", "passed_tests=1"].as_slice(),
            "............................................................",
        ),
        (
            "bun test",
            "\
bun test v1.2.0
tests/math.test.ts:
(pass) adds numbers [1.00ms]
(pass) subtracts numbers [1.00ms]
 2 pass
 0 fail
 4 expect() calls
Ran 2 tests across 1 files. [12.00ms]
",
            ["bun_test=8"].as_slice(),
            "(pass) adds numbers",
        ),
        (
            "uv run pytest -q",
            "\
........................................                              [100%]
40 passed, 2 skipped in 0.42s
",
            ["pytest_progress=1", "passed_tests=1"].as_slice(),
            "........................................",
        ),
        (
            "cargo nextest run",
            "\
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
------------
 Nextest run ID 123
    Starting 3 tests across 1 binary
        PASS [   0.001s] prodex_context tests::alpha
        PASS [   0.001s] prodex_context tests::beta
        PASS [   0.001s] prodex_context tests::gamma
------------
     Summary [   0.010s] 3 tests run: 3 passed, 0 skipped
",
            ["finished=1", "nextest_pass=3", "nextest_summary=1"].as_slice(),
            "prodex_context tests::alpha",
        ),
        (
            "swift test",
            "\
Building for debugging...
Build complete! (0.12s)
Test Suite 'All tests' started at 2026-05-05 00:00:00.000
Test Suite 'ProdexTests.xctest' passed at 2026-05-05 00:00:00.100.
Test Case '-[ProdexTests testAlpha]' passed (0.001 seconds).
Test Case '-[ProdexTests testBeta]' passed (0.001 seconds).
Test Suite 'All tests' passed at 2026-05-05 00:00:00.101.
Executed 2 tests, with 0 failures (0 unexpected) in 0.002 seconds
",
            ["swift_test=6"].as_slice(),
            "testAlpha",
        ),
        (
            "zig build test",
            "\
test
└─ run test
   └─ zig test Debug native 2 passed 0 skipped
Build Summary: 3/3 steps succeeded; 2/2 tests passed
",
            ["zig_test=4"].as_slice(),
            "zig test Debug native",
        ),
    ];

    for (command, input, expected_labels, omitted_line) in cases {
        let report = compact_successful_command_output_with_options(
            input,
            &CommandSuccessOutputCompactOptions {
                command: Some(command.to_string()),
                exit_code: Some(0),
                min_lines_to_compact: 1,
                max_touched_files: 8,
                max_key_lines: 6,
                max_line_chars: 160,
            },
        );

        assert!(report.compacted, "{command}: {}", report.output);
        assert!(!report.failure_suspected, "{command}");
        assert_eq!(report.critical_signals.total(), 0, "{command}");
        assert!(
            report.output.contains("noise:"),
            "{command}: {}",
            report.output
        );
        assert!(
            report.output.contains(command),
            "{command}: {}",
            report.output
        );
        for label in expected_labels {
            assert!(
                report.output.contains(label),
                "{command}: missing {label}\n{}",
                report.output
            );
        }
        assert!(!report.output.contains(omitted_line), "{command}");
        assert_no_critical_signal_loss(input, &report.output);
    }
}

#[test]
fn successful_command_output_summary_refuses_new_tool_warning_and_failure_signals() {
    let cases = [
        (
            "cargo clippy --fix --allow-dirty",
            "\
    Checking prodex-context v0.1.0
warning: variable does not need to be mutable
    Finished `dev` profile [unoptimized] target(s) in 1.23s
",
            Some(0),
        ),
        (
            "cargo fmt --all --check",
            "\
Diff in /repo/src/lib.rs at line 1:
 fn main() {}
",
            Some(1),
        ),
        (
            "biome check .",
            "\
Checked 3 files in 10ms. No fixes applied.
Found 1 error.
",
            Some(0),
        ),
        (
            "oxlint .",
            "\
Found 1 warning and 0 errors.
Finished in 12ms on 42 files with 1 warning.
",
            Some(0),
        ),
        (
            "pytest -q",
            "\
..F                                                                      [100%]
1 failed, 2 passed in 0.12s
",
            Some(0),
        ),
        (
            "go test -v ./...",
            "\
=== RUN   TestAlpha
--- FAIL: TestAlpha (0.00s)
FAIL
",
            Some(0),
        ),
        (
            "pnpm test --reporter=dot",
            "\
..F.
1 failed, 3 passed in 0.12s
",
            Some(0),
        ),
        (
            "bun test",
            "\
bun test v1.2.0
tests/math.test.ts:
(fail) adds numbers [1.00ms]
 1 pass
 1 fail
",
            Some(0),
        ),
        (
            "uv run pytest -q",
            "\
Traceback (most recent call last):
  File \"tests/test_app.py\", line 1, in test_app
AssertionError: boom
",
            Some(0),
        ),
        (
            "cargo nextest run",
            "\
        FAIL [   0.001s] prodex_context tests::alpha
     Summary [   0.010s] 1 test run: 0 passed, 1 failed
",
            Some(0),
        ),
        (
            "swift test",
            "\
warning: 'prodex': found 1 file which is unhandled
Test Suite 'All tests' passed at 2026-05-05 00:00:00.101.
Executed 2 tests, with 0 failures (0 unexpected) in 0.002 seconds
",
            Some(0),
        ),
        (
            "zig build test",
            "\
warning: unreachable code
Build Summary: 3/3 steps succeeded; 2/2 tests passed
",
            Some(0),
        ),
    ];

    for (command, input, exit_code) in cases {
        let report = compact_successful_command_output_with_options(
            input,
            &CommandSuccessOutputCompactOptions {
                command: Some(command.to_string()),
                exit_code,
                min_lines_to_compact: 1,
                ..CommandSuccessOutputCompactOptions::default()
            },
        );

        assert!(!report.compacted, "{command}");
        assert!(report.failure_suspected, "{command}");
        assert_eq!(report.output, input, "{command}");
        assert_no_critical_signal_loss(input, &report.output);
    }
}

#[test]
fn successful_command_output_summary_allows_zero_failed_and_error_counts() {
    let mut input = String::new();
    for index in 0..24 {
        input.push_str(&format!("PASS tests/unit_{index}.test.ts\n"));
    }
    input.push_str("Test Suites: 24 passed, 0 failed, 24 total\n");
    input.push_str("Tests:       120 passed, 0 failed, 120 total\n");
    input.push_str("Error: 0\n");
    input.push_str("found 0 errors\n");

    let report = compact_successful_command_output_with_options(
        &input,
        &CommandSuccessOutputCompactOptions {
            command: Some("npm test".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(report.compacted);
    assert!(!report.failure_suspected);
    assert_eq!(report.critical_signals.total(), 0);
    assert!(
        report
            .output
            .contains("Test Suites: 24 passed, 0 failed, 24 total")
    );
    assert!(
        report
            .output
            .contains("Tests:       120 passed, 0 failed, 120 total")
    );
    assert!(report.output.contains("found 0 errors"));
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn successful_command_output_summary_keeps_nonzero_failed_and_error_counts_exact() {
    let input = "\
Test Suites: 23 passed, 1 failed, 24 total
Tests:       118 passed, 2 failed, 120 total
Errors: 1
";

    let report = compact_successful_command_output_with_options(
        input,
        &CommandSuccessOutputCompactOptions {
            command: Some("npm test".to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            ..CommandSuccessOutputCompactOptions::default()
        },
    );

    assert!(!report.compacted);
    assert!(report.failure_suspected);
    assert_eq!(report.output, input);
    assert_no_critical_signal_loss(input, &report.output);
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
