use super::*;

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
