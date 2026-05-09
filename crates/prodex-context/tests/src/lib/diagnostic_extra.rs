use super::*;

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
