use super::*;

pub(super) fn looks_like_rust_diagnostic_output(lines: &[&str]) -> bool {
    let (
        strong_signals,
        cargo_noise_signals,
        location_signals,
        backtrace_signals,
        exit_signals,
        clippy_signals,
    ) = rust_diagnostic_signal_counts(lines);
    if strong_signals > 0 {
        return strong_signals
            + cargo_noise_signals
            + location_signals
            + backtrace_signals
            + exit_signals
            + clippy_signals
            >= 2;
    }
    if backtrace_signals > 0 && location_signals > 0 {
        return true;
    }
    if exit_signals > 0 && (cargo_noise_signals > 0 || location_signals > 0) {
        return true;
    }
    cargo_noise_signals >= 4
}

fn rust_diagnostic_signal_counts(lines: &[&str]) -> (usize, usize, usize, usize, usize, usize) {
    let mut strong_signals = 0usize;
    let mut cargo_noise_signals = 0usize;
    let mut location_signals = 0usize;
    let mut backtrace_signals = 0usize;
    let mut exit_signals = 0usize;
    let mut clippy_signals = 0usize;
    for line in lines {
        if rust_diagnostic_severity(line).is_some()
            || rust_failed_test_name(line).is_some()
            || rust_failure_separator_name(line).is_some()
            || is_rust_panic_line(line)
            || is_rust_failure_summary_line(line)
        {
            strong_signals += 1;
        }
        if is_rust_location_line(line) {
            location_signals += 1;
        }
        if is_rust_backtrace_start(line) {
            backtrace_signals += 1;
        }
        if is_rust_exit_status_line(line) {
            exit_signals += 1;
        }
        if rust_noise_label(line).is_some() {
            cargo_noise_signals += 1;
        }
        if line.contains("clippy::") || line.contains("cargo clippy") {
            clippy_signals += 1;
        }
    }

    (
        strong_signals,
        cargo_noise_signals,
        location_signals,
        backtrace_signals,
        exit_signals,
        clippy_signals,
    )
}

pub(super) fn looks_like_diagnostic_output(lines: &[&str]) -> bool {
    let (strong_signals, location_signals, stack_signals, exit_signals, noise_signals) =
        diagnostic_signal_counts(lines);
    if lines
        .iter()
        .any(|line| is_junit_xml_failure_line(line) || is_eslint_diagnostic_line(line))
    {
        return true;
    }
    if strong_signals > 0 {
        return strong_signals + location_signals + stack_signals + exit_signals + noise_signals
            >= 2;
    }
    stack_signals > 0 && location_signals > 0
}

fn diagnostic_signal_counts(lines: &[&str]) -> (usize, usize, usize, usize, usize) {
    let mut strong_signals = 0usize;
    let mut location_signals = 0usize;
    let mut stack_signals = 0usize;
    let mut exit_signals = 0usize;
    let mut noise_signals = 0usize;

    for line in lines {
        if is_diagnostic_detection_start(line) {
            strong_signals += 1;
        }
        if count_file_location_signals(line) > 0 {
            location_signals += 1;
        }
        if is_stack_signal_line(line) {
            stack_signals += 1;
        }
        if is_rust_exit_status_line(line) {
            exit_signals += 1;
        }
        if diagnostic_noise_label(line).is_some() {
            noise_signals += 1;
        }
    }

    (
        strong_signals,
        location_signals,
        stack_signals,
        exit_signals,
        noise_signals,
    )
}

pub(super) fn looks_like_noisy_success_output(lines: &[&str]) -> bool {
    let non_empty = lines.iter().filter(|line| !line.trim().is_empty()).count();
    if non_empty < 8 {
        return false;
    }
    if lines.iter().any(|line| {
        is_error_signal_line(line)
            || is_test_failure_signal_line(line)
            || is_success_output_failure_signal_line(line)
            || is_success_output_warning_signal_line(line)
    }) {
        return false;
    }

    let success_signals = lines
        .iter()
        .filter(|line| noisy_success_label(line).is_some())
        .count();
    let key_lines = lines
        .iter()
        .filter(|line| is_noisy_success_key_line(line))
        .count();
    success_signals >= 4 || (key_lines > 0 && success_signals >= 2)
}

pub(super) fn rust_block_line_limit(options: &CommandOutputCompactOptions) -> usize {
    options.max_lines.max(24).saturating_div(5).clamp(8, 32)
}

pub(super) fn diagnostic_block_line_limit(options: &CommandOutputCompactOptions) -> usize {
    options.max_lines.max(24).saturating_div(5).clamp(8, 32)
}

pub(super) fn rust_noise_label(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if is_success_output_failure_signal_line(trimmed)
        || is_success_output_warning_signal_line(trimmed)
        || is_error_signal_line(trimmed)
        || is_test_failure_signal_line(trimmed)
    {
        return None;
    }

    #[cfg(feature = "mojo")]
    {
        return noisy_success_mojo_label(line)
            .and_then(rust_noise_label_from_mojo_code)
            .or_else(|| rust_compilation_noise_label(trimmed))
            .or_else(|| rust_test_noise_label(trimmed));
    }

    #[cfg(not(feature = "mojo"))]
    rust_compilation_noise_label(trimmed).or_else(|| rust_test_noise_label(trimmed))
}

fn rust_compilation_noise_label(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("Compiling ") {
        Some("compiling")
    } else if trimmed.starts_with("Checking ") {
        Some("checking")
    } else if trimmed.starts_with("Fresh ") {
        Some("fresh")
    } else if trimmed.starts_with("Documenting ") {
        Some("documenting")
    } else if trimmed.starts_with("Formatting ") {
        Some("formatting")
    } else if trimmed.starts_with("Fixing ") || trimmed.starts_with("Fixed ") {
        Some("cargo_fix")
    } else if trimmed.starts_with("Generated ") {
        Some("generated_docs")
    } else if trimmed.starts_with("Finished ") {
        Some("finished")
    } else {
        None
    }
}

fn rust_test_noise_label(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("Running ") {
        Some("running_targets")
    } else if trimmed.starts_with("Doc-tests ") {
        Some("doc_tests")
    } else if trimmed.starts_with("running ") && trimmed.ends_with(" tests") {
        Some("running_tests")
    } else if trimmed.starts_with("test ") && trimmed.contains(" ... ok") {
        Some("passed_tests")
    } else if trimmed.starts_with("PASS [") {
        Some("nextest_pass")
    } else if trimmed.starts_with("Summary [") && trimmed.contains(" passed") {
        Some("nextest_summary")
    } else if trimmed.starts_with("test result: ok") {
        Some("test_result_ok")
    } else {
        None
    }
}

#[cfg(feature = "mojo")]
fn rust_noise_label_from_mojo_code(code: i64) -> Option<&'static str> {
    match code {
        1 => Some("compiling"),
        2 => Some("checking"),
        3 => Some("fresh"),
        4 => Some("documenting"),
        5 => Some("formatting"),
        6 => Some("cargo_fix"),
        7 => Some("generated_docs"),
        8 => Some("finished"),
        9 => Some("running_targets"),
        10 => Some("doc_tests"),
        11 => Some("running_tests"),
        12 => Some("passed_tests"),
        13 => Some("nextest_pass"),
        14 => Some("nextest_summary"),
        15 => Some("test_result_ok"),
        _ => None,
    }
}

pub(super) fn diagnostic_noise_label(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.starts_with("> ") {
        Some("npm_script")
    } else if lower.starts_with("collecting ") {
        Some("pytest_collecting")
    } else if lower.starts_with("collected ") && lower.contains(" item") {
        Some("pytest_collected")
    } else if is_pytest_progress_line(trimmed) {
        Some("pytest_progress")
    } else {
        noisy_success_label(line)
    }
}

pub(super) fn noisy_success_label(line: &str) -> Option<&'static str> {
    #[cfg(feature = "mojo")]
    {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if is_success_output_failure_signal_line(trimmed)
            && !is_diagnostic_failure_summary_line(trimmed)
            || is_success_output_warning_signal_line(trimmed)
        {
            return None;
        }
        return noisy_success_framework_label(trimmed, &lower)
            .or_else(|| noisy_success_mojo_label(line).and_then(noisy_success_label_from_mojo_code))
            .or_else(|| rust_noise_label(line))
            .or_else(|| noisy_success_language_label(trimmed, &lower))
            .or_else(|| noisy_success_go_test_label(trimmed, &lower))
            .or_else(|| noisy_success_test_summary_label(trimmed, &lower))
            .or_else(|| noisy_success_build_label(&lower))
            .or_else(|| noisy_success_package_label(&lower))
            .or_else(|| noisy_success_quality_label(trimmed, &lower));
    }

    #[cfg(not(feature = "mojo"))]
    {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if is_success_output_failure_signal_line(trimmed)
            && !is_diagnostic_failure_summary_line(trimmed)
            || is_success_output_warning_signal_line(trimmed)
        {
            return None;
        }

        noisy_success_framework_label(trimmed, &lower)
            .or_else(|| rust_noise_label(line))
            .or_else(|| noisy_success_language_label(trimmed, &lower))
            .or_else(|| noisy_success_go_test_label(trimmed, &lower))
            .or_else(|| noisy_success_test_summary_label(trimmed, &lower))
            .or_else(|| noisy_success_build_label(&lower))
            .or_else(|| noisy_success_package_label(&lower))
            .or_else(|| noisy_success_quality_label(trimmed, &lower))
    }
}

#[cfg(feature = "mojo")]
fn noisy_success_mojo_label(line: &str) -> Option<i64> {
    prodex_mojo_core::context::classify_noisy_success_line(line)
        .unwrap_or_else(|error| panic!("Mojo command-output classification failed: {error:?}"))
}

#[cfg(feature = "mojo")]
fn noisy_success_label_from_mojo_code(code: i64) -> Option<&'static str> {
    Some(match code {
        1 => "compiling",
        2 => "checking",
        3 => "fresh",
        4 => "documenting",
        5 => "formatting",
        6 => "cargo_fix",
        7 => "generated_docs",
        8 => "finished",
        9 => "running_targets",
        10 => "doc_tests",
        11 => "running_tests",
        12 => "passed_tests",
        13 => "nextest_pass",
        14 => "nextest_summary",
        15 => "test_result_ok",
        16 => "typecheck_summary",
        17 => "vite",
        18 => "next",
        19 => "dot_progress",
        20 => "bun_test",
        21 => "cypress",
        22 => "zig_test",
        23 => "passed_suites",
        24 => "go_test_ok",
        25 => "go_test_no_files",
        26 => "go_test_run",
        27 => "go_test_pause",
        28 => "go_test_cont",
        29 => "go_test_pass",
        30 => "go_test_skip",
        31 => "go_test_pass_summary",
        32 => "test_suites",
        33 => "test_cases",
        34 => "snapshots",
        35 => "test_files",
        36 => "test_duration",
        37 => "test_time",
        38 => "test_runner_summary",
        39 => "done",
        40 => "packages_added",
        41 => "packages_audited",
        42 => "cargo_index",
        43 => "cargo_lock",
        44 => "cargo_download",
        45 => "package_progress",
        46 => "packages_up_to_date",
        47 => "python_packages",
        48 => "vulnerability_summary",
        49 => "formatter_summary",
        50 => "compile_summary",
        51 => "build_summary",
        52 => "passed_tests",
        53 => "pytest_progress",
        _ => return None,
    })
}

fn noisy_success_framework_label(trimmed: &str, lower: &str) -> Option<&'static str> {
    if is_coverage_noise_line(trimmed, lower) {
        Some("coverage")
    } else if is_gradle_test_success_line(trimmed, lower) {
        Some("gradle_test")
    } else if is_maven_test_success_line(trimmed, lower) {
        Some("maven_test")
    } else if is_package_install_success_line(lower) {
        Some("package_install")
    } else if is_docker_buildx_success_line(lower) {
        Some("docker_buildx")
    } else if is_bazel_test_success_line(lower) {
        Some("bazel_test")
    } else if is_junit_xml_success_line(trimmed, lower) {
        Some("junit_xml")
    } else if is_swift_test_success_line(lower) {
        Some("swift_test")
    } else if is_playwright_success_line(trimmed, lower) {
        Some("playwright")
    } else if is_biome_success_summary_line(lower) {
        Some("biome_summary")
    } else if is_oxlint_success_summary_line(lower) {
        Some("oxlint_summary")
    } else {
        None
    }
}

fn noisy_success_language_label(trimmed: &str, lower: &str) -> Option<&'static str> {
    if is_typescript_success_line(trimmed, lower) {
        Some("typecheck_summary")
    } else if is_vite_success_line(trimmed, lower) {
        Some("vite")
    } else if is_next_success_line(trimmed, lower) {
        Some("next")
    } else if is_dot_reporter_success_line(trimmed) {
        Some("dot_progress")
    } else if is_bun_test_success_line(trimmed, lower) {
        Some("bun_test")
    } else if is_cypress_success_line(trimmed, lower) {
        Some("cypress")
    } else if is_zig_test_success_line(lower) {
        Some("zig_test")
    } else {
        None
    }
}

fn noisy_success_go_test_label(trimmed: &str, lower: &str) -> Option<&'static str> {
    if trimmed.starts_with("PASS ") {
        Some("passed_suites")
    } else if lower.starts_with("ok ") && lower.split_whitespace().count() >= 2 {
        Some("go_test_ok")
    } else if lower.starts_with("? ") && lower.contains("[no test files]") {
        Some("go_test_no_files")
    } else if lower.starts_with("=== run ") {
        Some("go_test_run")
    } else if lower.starts_with("=== pause ") {
        Some("go_test_pause")
    } else if lower.starts_with("=== cont ") {
        Some("go_test_cont")
    } else if lower.starts_with("--- pass: ") {
        Some("go_test_pass")
    } else if lower.starts_with("--- skip: ") {
        Some("go_test_skip")
    } else if trimmed == "PASS" {
        Some("go_test_pass_summary")
    } else {
        None
    }
}

fn noisy_success_test_summary_label(trimmed: &str, lower: &str) -> Option<&'static str> {
    noisy_success_test_count_label(trimmed, lower)
        .or_else(|| noisy_success_test_timing_label(lower))
}

fn noisy_success_test_count_label(trimmed: &str, lower: &str) -> Option<&'static str> {
    if lower.starts_with("test suites:") && lower.contains("passed") {
        Some("test_suites")
    } else if lower.starts_with("tests:") && lower.contains("passed") {
        Some("test_cases")
    } else if lower.starts_with("summary [") && lower.contains(" passed") {
        Some("nextest_summary")
    } else if trimmed.starts_with("PASS [") {
        Some("nextest_pass")
    } else if lower.starts_with("snapshots:")
        && (lower.contains("passed") || lower.contains("0 total"))
    {
        Some("snapshots")
    } else if lower.starts_with("test files") && lower.contains("passed") {
        Some("test_files")
    } else {
        None
    }
}

fn noisy_success_test_timing_label(lower: &str) -> Option<&'static str> {
    if lower.starts_with("duration") {
        Some("test_duration")
    } else if lower.starts_with("time:") {
        Some("test_time")
    } else if lower.starts_with("ran all test suites") {
        Some("test_runner_summary")
    } else if lower.starts_with("done in ") {
        Some("done")
    } else {
        None
    }
}

fn noisy_success_build_label(lower: &str) -> Option<&'static str> {
    noisy_success_generic_build_label(lower)
        .or_else(|| noisy_success_gradle_build_label(lower))
        .or_else(|| noisy_success_container_build_label(lower))
        .or_else(|| noisy_success_runner_build_label(lower))
}

fn noisy_success_generic_build_label(lower: &str) -> Option<&'static str> {
    if lower.starts_with("build successful")
        || lower.starts_with("build success")
        || lower.starts_with("[info] build success")
        || lower.contains(" build success")
    {
        Some("build_success")
    } else if lower.starts_with("[info] --- ") || lower.starts_with("> task ") {
        Some("build_steps")
    } else if lower.starts_with("info: analyzed target")
        || lower.starts_with("info: found ")
        || lower.starts_with("info: elapsed time:")
    {
        Some("bazel_steps")
    } else if (lower.starts_with("target ") && lower.contains("up-to-date"))
        || lower.starts_with("info: build completed successfully")
    {
        Some("bazel_summary")
    } else if lower.contains("successfully ran target")
        || lower.contains("successfully ran targets")
        || lower.starts_with("nx successfully ran")
    {
        Some("nx_summary")
    } else if lower.starts_with("tasks:") && lower.contains("successful") {
        Some("turbo_summary")
    } else {
        None
    }
}

fn noisy_success_gradle_build_label(lower: &str) -> Option<&'static str> {
    (lower.contains("actionable tasks:") || lower.contains("actionable task:"))
        .then_some("gradle_tasks")
}

fn noisy_success_container_build_label(lower: &str) -> Option<&'static str> {
    if lower.starts_with("[info] total time:")
        || lower.starts_with("[info] finished at:")
        || lower.starts_with("[info] tests run:")
    {
        Some("maven_summary")
    } else if lower.starts_with("=> ")
        || lower.starts_with("=>=> ")
        || lower.starts_with("#") && (lower.contains(" done ") || lower.ends_with(" done"))
    {
        Some("docker_steps")
    } else if lower.starts_with("[+] running ")
        || (lower.starts_with("container ") && docker_compose_success_state(lower))
        || (lower.starts_with("network ") && lower.contains("created"))
        || (lower.starts_with("volume ") && lower.contains("created"))
    {
        Some("docker_compose")
    } else if lower.starts_with("successfully built ")
        || lower.starts_with("successfully tagged ")
        || lower.contains("writing image sha256:")
        || lower.contains("naming to ")
    {
        Some("docker_summary")
    } else {
        None
    }
}

fn noisy_success_runner_build_label(lower: &str) -> Option<&'static str> {
    if lower.starts_with("running ") && lower.contains(" tests using ") {
        Some("playwright_running")
    } else if lower.contains(" passed (") && lower.chars().any(|ch| ch.is_ascii_digit()) {
        Some("test_summary")
    } else {
        None
    }
}

fn noisy_success_package_label(lower: &str) -> Option<&'static str> {
    noisy_success_registry_package_label(lower)
        .or_else(|| noisy_success_installer_package_label(lower))
}

fn noisy_success_registry_package_label(lower: &str) -> Option<&'static str> {
    if lower.starts_with("added ") && lower.contains(" package") {
        Some("packages_added")
    } else if lower.starts_with("audited ") && lower.contains(" package") {
        Some("packages_audited")
    } else if lower.starts_with("updating crates.io index") {
        Some("cargo_index")
    } else if lower.starts_with("locking ") && lower.contains(" package") {
        Some("cargo_lock")
    } else if lower.starts_with("downloading crates") || lower.starts_with("downloaded ") {
        Some("cargo_download")
    } else if lower.starts_with("packages: ") || lower.starts_with("progress: resolved") {
        Some("package_progress")
    } else if lower.starts_with("lockfile is up to date") || lower.starts_with("already up to date")
    {
        Some("packages_up_to_date")
    } else {
        None
    }
}

fn noisy_success_installer_package_label(lower: &str) -> Option<&'static str> {
    if lower.starts_with("requirement already satisfied")
        || lower.starts_with("successfully installed")
        || lower.starts_with("installing collected packages")
        || (lower.starts_with("resolved ") && lower.contains(" package"))
        || (lower.starts_with("prepared ") && lower.contains(" package"))
        || (lower.starts_with("installed ") && lower.contains(" package"))
    {
        Some("python_packages")
    } else if lower == "up to date" || lower.starts_with("up to date in ") {
        Some("packages_up_to_date")
    } else if lower.starts_with("found 0 vulnerabilities") {
        Some("vulnerability_summary")
    } else {
        None
    }
}

fn noisy_success_quality_label(trimmed: &str, lower: &str) -> Option<&'static str> {
    if lower.starts_with("all files pass")
        || lower.contains("all matched files use prettier code style")
        || lower.contains("eslint found no problems")
        || lower.starts_with("all checks passed")
    {
        Some("formatter_summary")
    } else if lower.starts_with("success: no issues found")
        || lower.starts_with("found 0 errors")
        || lower.starts_with("found 0 warnings")
        || lower.starts_with("found 0 issues")
    {
        Some("typecheck_summary")
    } else if lower.starts_with("built in ") || lower.contains(" built in ") {
        Some("build_summary")
    } else if lower.starts_with("compiled successfully") {
        Some("compile_summary")
    } else if (lower.starts_with("tests/") && lower.contains(" passed"))
        || (lower.contains("::test_") && lower.ends_with(" passed"))
        || is_pytest_success_summary_line(lower)
    {
        Some("passed_tests")
    } else if is_pytest_progress_line(trimmed) {
        Some("pytest_progress")
    } else {
        None
    }
}

#[cfg(test)]
mod noisy_success_leaf_tests {
    use super::*;

    #[test]
    fn dot_reporter_leaf_matches_rust_oracle_and_label_path() {
        for line in ["....", ".....", "...", "  ....  ", "....x", "....🙂"] {
            let trimmed = line.trim();
            let expected = is_dot_reporter_success_line_rust(trimmed);
            assert_eq!(is_dot_reporter_success_line(trimmed), expected, "{line:?}");
            assert_eq!(
                noisy_success_label(line).is_some_and(|label| label == "dot_progress"),
                expected,
                "{line:?}"
            );
        }
    }
}
