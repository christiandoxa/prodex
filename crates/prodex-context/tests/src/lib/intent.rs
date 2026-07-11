use super::*;

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
