use super::*;

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
