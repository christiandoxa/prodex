use super::*;

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
