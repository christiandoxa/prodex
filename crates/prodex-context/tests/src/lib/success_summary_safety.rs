use super::*;

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
