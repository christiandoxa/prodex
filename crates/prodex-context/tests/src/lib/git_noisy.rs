use super::*;

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
