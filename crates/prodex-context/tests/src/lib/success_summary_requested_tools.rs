use super::*;

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
