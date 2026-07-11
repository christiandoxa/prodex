use super::*;

#[path = "success_summary_requested_tools.rs"]
mod requested_tools;
#[path = "success_summary_safety.rs"]
mod safety;

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
