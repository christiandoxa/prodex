use super::super::*;
#[cfg(feature = "mojo")]
use prodex_mojo_core::context::{
    CONTEXT_OUTPUT_DIAGNOSTIC_FAILURE, CONTEXT_OUTPUT_DIAGNOSTIC_SUCCESS,
    CONTEXT_OUTPUT_DIAGNOSTIC_TARGET, CONTEXT_OUTPUT_EXCEPTION, CONTEXT_OUTPUT_FAILURE,
    CONTEXT_OUTPUT_NOISY_KEY,
    CONTEXT_OUTPUT_TYPESCRIPT, CONTEXT_OUTPUT_WARNING, CommandOutputLineClassification,
    classify_command_output_line,
};

#[cfg(feature = "mojo")]
const MOJO_OUTPUT_LABELS: &[&str] = &[
    "",
    "coverage",
    "gradle_test",
    "maven_test",
    "package_install",
    "docker_buildx",
    "bazel_test",
    "junit_xml",
    "swift_test",
    "playwright",
    "biome_summary",
    "oxlint_summary",
    "compiling",
    "checking",
    "fresh",
    "documenting",
    "formatting",
    "cargo_fix",
    "generated_docs",
    "finished",
    "running_targets",
    "doc_tests",
    "running_tests",
    "passed_tests",
    "nextest_pass",
    "nextest_summary",
    "test_result_ok",
    "typecheck_summary",
    "vite",
    "next",
    "dot_progress",
    "bun_test",
    "cypress",
    "zig_test",
    "passed_suites",
    "go_test_ok",
    "go_test_no_files",
    "go_test_run",
    "go_test_pause",
    "go_test_cont",
    "go_test_pass",
    "go_test_skip",
    "go_test_pass_summary",
    "test_suites",
    "test_cases",
    "snapshots",
    "test_files",
    "test_duration",
    "test_time",
    "test_runner_summary",
    "done",
    "build_success",
    "build_steps",
    "bazel_steps",
    "bazel_summary",
    "nx_summary",
    "turbo_summary",
    "gradle_tasks",
    "maven_summary",
    "docker_steps",
    "docker_compose",
    "docker_summary",
    "playwright_running",
    "test_summary",
    "packages_added",
    "packages_audited",
    "cargo_index",
    "cargo_lock",
    "cargo_download",
    "package_progress",
    "packages_up_to_date",
    "python_packages",
    "vulnerability_summary",
    "formatter_summary",
    "build_summary",
    "compile_summary",
    "pytest_progress",
    "npm_script",
    "pytest_collecting",
    "pytest_collected",
];

#[cfg(feature = "mojo")]
fn mojo_output_line(line: &str) -> CommandOutputLineClassification {
    classify_command_output_line(line)
        .unwrap_or_else(|error| panic!("Mojo command-output classification failed: {error:?}"))
}

#[cfg(feature = "mojo")]
pub(crate) fn mojo_output_label(label: i64) -> Option<&'static str> {
    usize::try_from(label)
        .ok()
        .and_then(|index| MOJO_OUTPUT_LABELS.get(index).copied())
        .filter(|label| !label.is_empty())
}

#[cfg(feature = "mojo")]
pub(crate) fn command_output_line_classification(line: &str) -> CommandOutputLineClassification {
    mojo_output_line(line)
}

#[cfg(feature = "mojo")]
pub(crate) fn is_noisy_success_key_line(line: &str) -> bool {
    mojo_output_line(line).flags & CONTEXT_OUTPUT_NOISY_KEY != 0
}

#[cfg(feature = "mojo")]
pub(crate) fn is_success_output_failure_signal_line(line: &str) -> bool {
    mojo_output_line(line).flags & CONTEXT_OUTPUT_FAILURE != 0
}

#[cfg(feature = "mojo")]
pub(crate) fn is_success_output_warning_signal_line(line: &str) -> bool {
    mojo_output_line(line).flags & CONTEXT_OUTPUT_WARNING != 0
}

#[cfg(feature = "mojo")]
pub(crate) fn is_diagnostic_key_line(line: &str) -> bool {
    let flags = mojo_output_line(line).flags;
    flags
        & (CONTEXT_OUTPUT_DIAGNOSTIC_SUCCESS
            | CONTEXT_OUTPUT_DIAGNOSTIC_FAILURE
            | CONTEXT_OUTPUT_NOISY_KEY)
        != 0
}

#[cfg(feature = "mojo")]
pub(crate) fn is_diagnostic_failure_summary_line(line: &str) -> bool {
    mojo_output_line(line).flags & CONTEXT_OUTPUT_DIAGNOSTIC_FAILURE != 0
}

#[cfg(feature = "mojo")]
pub(crate) fn is_diagnostic_block_start(line: &str) -> bool {
    let flags = mojo_output_line(line).flags;
    flags & CONTEXT_OUTPUT_DIAGNOSTIC_TARGET != 0
        || is_node_stack_error_line(line)
        || is_test_failure_signal_line(line)
        || is_stack_signal_line(line)
        || is_error_signal_line(line)
}

#[cfg(feature = "mojo")]
pub(crate) fn is_typescript_diagnostic_line(line: &str) -> bool {
    mojo_output_line(line).flags & CONTEXT_OUTPUT_TYPESCRIPT != 0
}

#[cfg(feature = "mojo")]
pub(crate) fn is_exception_signal_line(line: &str) -> bool {
    mojo_output_line(line).flags & CONTEXT_OUTPUT_EXCEPTION != 0
}
