use super::*;

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
