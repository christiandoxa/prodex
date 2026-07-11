use super::*;

mod counts;
mod rust;

pub(crate) use counts::*;
pub(crate) use rust::*;

pub(super) fn docker_compose_success_state(lower: &str) -> bool {
    lower.contains(" started")
        || lower.contains(" running")
        || lower.contains(" healthy")
        || lower.contains(" created")
        || lower.contains(" done")
        || lower.contains(" pulled")
}

pub(super) fn is_dot_reporter_success_line(trimmed: &str) -> bool {
    trimmed.len() >= 4 && trimmed.chars().all(|ch| ch == '.')
}

pub(super) fn is_bun_test_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("bun test v")
        || lower.starts_with("(pass) ")
        || lower.starts_with("ran ") && lower.contains(" tests across ") && lower.contains(" file")
        || zero_count_summary_line(lower, "fail")
        || is_count_word_line(lower, "pass")
        || lower.contains(" expect() call")
        || trimmed.ends_with(".test.ts:")
        || trimmed.ends_with(".test.tsx:")
        || trimmed.ends_with(".test.js:")
        || trimmed.ends_with(".test.jsx:")
}

pub(super) fn is_swift_test_success_line(lower: &str) -> bool {
    lower.starts_with("build complete!")
        || (lower.starts_with("test suite ") || lower.contains(" test suite "))
            && lower.contains(" passed at ")
        || (lower.starts_with("test case ") || lower.contains(" test case "))
            && lower.contains(" passed (")
        || lower.starts_with("executed ")
            && lower.contains(" tests")
            && lower.contains("with 0 failures")
}

pub(super) fn is_zig_test_success_line(lower: &str) -> bool {
    lower == "test"
        || lower.starts_with("run test")
        || lower.contains(" run test")
        || lower.contains(" zig test ") && lower.contains(" passed")
        || lower.contains(" steps succeeded")
        || lower.contains(" tests passed")
        || lower.starts_with("build summary:")
            && lower.contains("succeeded")
            && !has_nonzero_summary_count(lower, &["failed", "failures", "error", "errors"])
}

pub(super) fn is_gradle_test_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("> task ") && lower.contains(":test") && !lower.ends_with(" failed")
        || lower.contains(" > ") && (lower.ends_with(" passed") || lower.ends_with(" skipped"))
        || lower.starts_with("test run finished after")
        || lower.starts_with("[") && lower.contains(" tests successful")
        || lower.starts_with("[") && lower.contains(" tests skipped")
        || lower.ends_with(" tests completed")
        || lower.contains(" tests completed, 0 failed")
        || trimmed == "BUILD SUCCESSFUL"
}

pub(super) fn is_maven_test_success_line(_trimmed: &str, lower: &str) -> bool {
    let info_body = lower.strip_prefix("[info]").map(str::trim_start);
    lower.starts_with("[info] running ")
        || lower.starts_with("[info] results:")
        || lower.starts_with("[info] surefire report directory:")
        || lower.starts_with("[info] tests run:")
            && !has_nonzero_summary_count(lower, &["failures", "errors"])
        || lower.starts_with("[info] t e s t s")
        || info_body.is_some_and(|body| {
            body.len() >= 8 && body.chars().all(|ch| ch == '-' || ch.is_ascii_whitespace())
        })
}

pub(super) fn is_package_install_success_line(lower: &str) -> bool {
    lower.starts_with("yarn install v")
        || lower.starts_with("[1/4] resolving packages")
        || lower.starts_with("[2/4] fetching packages")
        || lower.starts_with("[3/4] linking dependencies")
        || lower.starts_with("[4/4] building fresh packages")
        || lower.starts_with("success saved lockfile")
        || lower.starts_with("success already up-to-date")
        || lower.starts_with("saved lockfile")
        || lower.starts_with("bun install v")
        || lower.starts_with("resolved, downloaded and extracted")
        || lower.contains(" packages installed")
        || lower.starts_with("scope: all ") && lower.contains("workspace project")
        || lower.starts_with("done in ") && lower.contains(" using pnpm ")
}

pub(super) fn is_docker_buildx_success_line(lower: &str) -> bool {
    lower.starts_with('#')
        && (lower.contains(" building with ")
            || lower.contains(" transferring ")
            || lower.contains(" exporting ")
            || lower.contains(" importing cache manifest")
            || lower.contains(" resolving provenance")
            || lower.contains(" writing image sha256:")
            || lower.contains(" naming to ")
            || lower.contains(" pushing layers")
            || lower.contains(" pushing manifest")
            || lower.ends_with(" cached"))
}

pub(super) fn is_bazel_test_success_line(lower: &str) -> bool {
    lower.starts_with("//") && (lower.contains(" passed in ") || lower.ends_with(" passed"))
        || lower.starts_with("executed ")
            && lower.contains(" out of ")
            && lower.contains(" tests")
            && (lower.contains(" tests pass") || lower.contains(" test passes"))
        || lower.starts_with("info: found ") && lower.contains(" test target")
        || lower.starts_with("info: ") && lower.contains(" processes:")
}

pub(super) fn zero_count_summary_line(lower: &str, word: &str) -> bool {
    lower.match_indices(word).any(|(index, matched)| {
        count_after_word(lower, index + matched.len()).or_else(|| count_before_word(lower, index))
            == Some(0)
    })
}

pub(super) fn is_count_word_line(lower: &str, expected_word: &str) -> bool {
    let mut words = lower.split_whitespace();
    let Some(count) = words.next() else {
        return false;
    };
    let Some(word) = words.next() else {
        return false;
    };
    words.next().is_none() && count.chars().all(|ch| ch.is_ascii_digit()) && word == expected_word
}

pub(super) fn is_typescript_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("project '") && lower.contains(" is up to date")
        || lower.starts_with("building project '")
        || lower.starts_with("updating unchanged output timestamps")
        || lower.starts_with("projects in this build:")
        || looks_like_typescript_project_line(trimmed)
}

pub(super) fn looks_like_typescript_project_line(trimmed: &str) -> bool {
    let trimmed = trimmed.trim_matches(|ch| matches!(ch, '\'' | '"' | '`' | ',' | ';'));
    trimmed.ends_with("tsconfig.json") || trimmed.ends_with("tsconfig.tsbuildinfo")
}

pub(super) fn is_vite_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("vite ") && lower.contains(" building for production")
        || lower == "transforming..."
        || lower == "rendering chunks..."
        || lower == "computing gzip size..."
        || lower.contains(" modules transformed")
        || lower.starts_with("dist/") && (lower.contains(" kb") || lower.contains("gzip:"))
        || trimmed.starts_with('✓') && lower.contains("built in ")
}

pub(super) fn is_next_success_line(trimmed: &str, lower: &str) -> bool {
    lower.starts_with("▲ next.js")
        || lower.starts_with("next.js ")
        || lower.starts_with("creating an optimized production build")
        || lower.starts_with("compiled successfully")
        || lower.contains("compiled successfully")
        || lower.starts_with("linting and checking validity of types")
        || lower.starts_with("collecting page data")
        || lower.starts_with("generating static pages")
        || lower.starts_with("finalizing page optimization")
        || lower.starts_with("collecting build traces")
        || lower.starts_with("route ") && lower.contains("first load js")
        || lower.starts_with("+ first load js")
        || lower.contains("first load js shared by all")
        || is_next_route_table_line(trimmed, lower)
}

pub(super) fn is_next_route_table_line(trimmed: &str, lower: &str) -> bool {
    matches!(
        trimmed.chars().next(),
        Some('┌' | '├' | '└' | '│' | '○' | '●' | 'ƒ' | '+')
    ) && (lower.contains(" kb") || lower.contains("first load js") || lower.contains("static"))
}

pub(super) fn is_coverage_noise_line(trimmed: &str, lower: &str) -> bool {
    if lower.starts_with("coverage summary")
        || lower.starts_with("all files")
        || lower.starts_with("statements")
        || lower.starts_with("branches")
        || lower.starts_with("functions")
        || lower.starts_with("lines")
        || lower.contains(" coverage: platform ")
        || lower.starts_with("name ") && lower.contains(" stmts ") && lower.contains(" cover")
        || lower.starts_with("total ") && lower.contains('%')
        || lower.starts_with("required test coverage") && lower.contains(" reached")
        || lower.starts_with("coverage html written")
        || lower.starts_with("coverage xml written")
        || lower.starts_with("coverage json written")
    {
        return true;
    }
    if looks_like_pytest_coverage_table_row(trimmed, lower) {
        return true;
    }
    trimmed.contains('|')
        && (lower.contains("% stmts")
            || lower.contains("% branch")
            || lower.contains("% funcs")
            || lower.contains("% lines")
            || lower.contains("uncovered line"))
}

pub(super) fn looks_like_pytest_coverage_table_row(trimmed: &str, lower: &str) -> bool {
    let mut columns = lower.split_whitespace();
    let Some(path) = columns.next() else {
        return false;
    };
    looks_like_location_path(path)
        && lower.contains('%')
        && trimmed
            .split_whitespace()
            .skip(1)
            .filter(|column| {
                column
                    .trim_end_matches('%')
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '.')
            })
            .count()
            >= 3
}

pub(super) fn is_junit_xml_success_line(trimmed: &str, lower: &str) -> bool {
    (trimmed.starts_with("<testsuite") || trimmed.starts_with("<testsuites"))
        && lower.contains("tests=")
        && (lower.contains("failures=\"0\"")
            || lower.contains("failures='0'")
            || lower.contains("failures=0"))
        && (lower.contains("errors=\"0\"")
            || lower.contains("errors='0'")
            || lower.contains("errors=0"))
}

pub(crate) fn is_junit_xml_failure_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    trimmed.starts_with("<failure")
        || trimmed.starts_with("<error")
        || ((trimmed.starts_with("<testsuite") || trimmed.starts_with("<testsuites"))
            && (xmlish_nonzero_attr(&lower, "failures") || xmlish_nonzero_attr(&lower, "errors")))
}

pub(super) fn xmlish_nonzero_attr(lower: &str, name: &str) -> bool {
    for needle in [
        format!("{name}=\""),
        format!("{name}='"),
        format!("{name}="),
    ] {
        if let Some(after) = lower.split(&needle).nth(1) {
            let digits = after
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if matches!(digits.parse::<usize>(), Ok(count) if count > 0) {
                return true;
            }
        }
    }
    false
}

pub(super) fn is_playwright_success_line(trimmed: &str, lower: &str) -> bool {
    (lower.starts_with("running ") && lower.contains(" tests using "))
        || (lower.contains(" passed (") && lower.chars().any(|ch| ch.is_ascii_digit()))
        || lower.starts_with("slow test file:")
        || trimmed.starts_with('✓')
}

pub(super) fn is_cypress_success_line(trimmed: &str, lower: &str) -> bool {
    lower.contains("all specs passed")
        || lower.starts_with("spec")
        || lower.starts_with("tests")
        || lower.starts_with("passing")
        || trimmed.starts_with('✔')
}

pub(super) fn is_biome_success_summary_line(lower: &str) -> bool {
    (lower.starts_with("checked ")
        || lower.starts_with("formatted ")
        || lower.starts_with("linted "))
        && lower.contains(" file")
        && lower.contains(" in ")
        && (lower.contains("no fixes applied")
            || lower.contains("fixed ")
            || lower.contains("no issues found"))
        || lower == "no fixes applied."
        || lower.starts_with("fixed ") && lower.contains(" file")
}

pub(super) fn is_oxlint_success_summary_line(lower: &str) -> bool {
    lower.starts_with("finished in ")
        && lower.contains(" on ")
        && lower.contains(" file")
        && (lower.contains("0 warning")
            || lower.contains("0 error")
            || !lower.contains("warning") && !lower.contains("error"))
}

pub(super) fn is_pytest_progress_line(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(progress) = trimmed.split_whitespace().next() else {
        return false;
    };
    progress.len() >= 2
        && progress
            .chars()
            .all(|ch| matches!(ch, '.' | 's' | 'S' | 'x' | 'X'))
        && progress.chars().any(|ch| ch == '.')
        && trimmed
            .get(progress.len()..)
            .map(str::trim)
            .is_some_and(|rest| {
                rest.is_empty()
                    || rest.starts_with('[') && rest.ends_with(']') && rest.contains('%')
            })
}

pub(super) fn is_pytest_success_summary_line(lower: &str) -> bool {
    lower.contains(" passed")
        && lower.contains(" in ")
        && !lower.contains(" failed")
        && !lower.contains(" error")
        && !lower.contains("errors")
}

pub(super) fn is_diagnostic_key_line(line: &str) -> bool {
    is_diagnostic_success_summary_line(line)
        || is_diagnostic_failure_summary_line(line)
        || is_noisy_success_key_line(line)
}

pub(super) fn is_diagnostic_success_summary_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("test suites:")
        || lower.starts_with("tests:")
        || lower.starts_with("snapshots:")
        || lower.starts_with("test files")
        || lower.starts_with("ran all test suites")
        || lower.starts_with("found 0 vulnerabilities")
        || is_junit_xml_success_line(line.trim_start(), &lower)
        || lower.contains(" passed in ")
        || is_common_success_summary_line(&lower)
}

pub(super) fn is_diagnostic_failure_summary_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("test suites:") && has_nonzero_summary_count(&lower, &["failed"])
        || lower.starts_with("tests:") && has_nonzero_summary_count(&lower, &["failed"])
        || lower.contains(" failed, ") && has_nonzero_summary_count(&lower, &["failed"])
        || lower.starts_with("failed ")
            && !has_zero_only_summary_count(&lower, &["failed", "failures"])
        || lower.starts_with("error summary")
            && !has_zero_only_summary_count(&lower, &["error", "errors"])
        || is_junit_xml_failure_line(line)
}

pub(super) fn is_noisy_success_key_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    lower.starts_with("test suites:")
        || lower.starts_with("tests:")
        || lower.starts_with("snapshots:")
        || lower.starts_with("test files")
        || lower.starts_with("summary [") && lower.contains(" passed")
        || lower.starts_with("ran all test suites")
        || lower.starts_with("done in ")
        || lower.starts_with("added ")
        || lower.starts_with("audited ")
        || lower.starts_with("packages: ")
        || lower.starts_with("build successful")
        || lower.contains(" build success")
        || lower.starts_with("[info] build success")
        || lower.starts_with("info: build completed successfully")
        || (lower.starts_with("target ") && lower.contains("up-to-date"))
        || lower.contains("successfully ran target")
        || lower.contains("successfully ran targets")
        || (lower.starts_with("tasks:") && lower.contains("successful"))
        || lower.contains("actionable tasks:")
        || lower.contains("actionable task:")
        || is_gradle_test_success_line(line.trim_start(), &lower)
        || lower.starts_with("[info] tests run:")
        || is_maven_test_success_line(line.trim_start(), &lower)
        || lower.starts_with("successfully built ")
        || lower.starts_with("successfully tagged ")
        || lower.starts_with("[+] running ")
        || (lower.starts_with("container ") && docker_compose_success_state(&lower))
        || lower.contains("writing image sha256:")
        || lower.contains("naming to ")
        || is_docker_buildx_success_line(&lower)
        || lower.contains("all matched files use prettier code style")
        || lower.contains("eslint found no problems")
        || lower.starts_with("found 0 vulnerabilities")
        || lower == "up to date"
        || lower.starts_with("up to date in ")
        || lower.starts_with("lockfile is up to date")
        || lower.starts_with("already up to date")
        || is_package_install_success_line(&lower)
        || lower.starts_with("successfully installed")
        || (lower.starts_with("resolved ") && lower.contains(" package"))
        || (lower.starts_with("prepared ") && lower.contains(" package"))
        || (lower.starts_with("installed ") && lower.contains(" package"))
        || lower.starts_with("all files pass")
        || lower.starts_with("all checks passed")
        || lower.starts_with("success: no issues found")
        || lower.starts_with("found 0 errors")
        || lower.starts_with("found 0 issues")
        || lower.starts_with("built in ")
        || lower.contains(" passed in ")
        || is_bazel_test_success_line(&lower)
        || is_common_success_summary_line(&lower)
        || lower.starts_with("compiled successfully")
        || is_coverage_noise_line(line.trim_start(), &lower)
        || is_junit_xml_success_line(line.trim_start(), &lower)
        || is_playwright_success_line(line.trim_start(), &lower)
        || is_cypress_success_line(line.trim_start(), &lower)
}

pub(super) fn is_common_success_summary_line(lower: &str) -> bool {
    lower.starts_with("build successful")
        || lower.contains(" build success")
        || lower.starts_with("[info] build success")
        || lower.contains("actionable tasks:")
        || lower.contains("actionable task:")
        || lower.starts_with("[info] tests run:")
        || lower.starts_with("successfully built ")
        || lower.starts_with("successfully tagged ")
        || lower.starts_with("info: build completed successfully")
        || lower.contains("successfully ran target")
        || lower.contains("successfully ran targets")
        || (lower.starts_with("tasks:") && lower.contains("successful"))
        || (lower.starts_with("summary [") && lower.contains(" passed"))
        || lower.starts_with("success: no issues found")
        || lower.starts_with("found 0 errors")
        || lower.starts_with("all checks passed")
        || lower.contains("all matched files use prettier code style")
        || lower.contains("eslint found no problems")
        || (lower.starts_with("test files") && lower.contains("passed"))
        || lower.contains(" passed (")
        || lower.contains("all specs passed")
}

pub(super) fn is_success_output_failure_signal_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    lower.starts_with("build failure")
        || lower.starts_with("build failed")
        || lower.contains("build did not complete successfully")
        || lower.contains("build did not complete")
        || lower.starts_with("info: build failed")
        || lower.starts_with("failed:")
        || lower.starts_with("fail:")
        || lower.starts_with("--- fail:")
        || lower.starts_with("(fail) ")
        || lower.starts_with("failed tests:")
        || lower.starts_with("there were failing")
        || lower.starts_with("there were test failures")
        || lower.contains(" test failures")
        || lower.contains("tests failed")
        || lower.starts_with("type error")
        || lower.contains("failed to compile")
        || lower.contains("failed to load")
        || lower.contains("failed with")
        || lower.contains("execution failed")
        || lower.contains("failed to solve")
        || lower.contains("executor failed running")
        || lower.starts_with("> task ") && lower.ends_with(" failed")
        || lower.starts_with("//") && lower.contains(" failed")
        || lower.starts_with('#') && lower.contains(" error")
        || lower.starts_with("err_pnpm_")
        || lower.contains("required test coverage") && lower.contains("not reached")
        || is_nonzero_fail_count_line(&lower)
        || is_junit_xml_failure_line(line)
        || has_nonzero_summary_count(
            &lower,
            &["failed", "failures", "failing", "fails", "error", "errors"],
        )
}

pub(super) fn is_nonzero_fail_count_line(lower: &str) -> bool {
    lower.match_indices("fail").any(|(index, matched)| {
        let count = count_after_word(lower, index + matched.len())
            .or_else(|| count_before_word(lower, index));
        count.is_some_and(|count| count > 0)
            && lower
                .split_whitespace()
                .all(|word| word.chars().all(|ch| ch.is_ascii_digit()) || word == "fail")
    })
}

pub(super) fn is_success_output_warning_signal_line(line: &str) -> bool {
    let lower = line.trim_start().to_ascii_lowercase();
    if lower.is_empty() || has_zero_only_summary_count(&lower, &["warning", "warnings"]) {
        return false;
    }

    lower.starts_with("warning")
        || lower.starts_with("warn ")
        || lower.starts_with("warn:")
        || lower.starts_with("[warning]")
        || lower.starts_with("[warn]")
        || lower.starts_with("npm warn")
        || lower.starts_with("pnpm warn")
        || lower.starts_with("yarn warning")
        || lower.starts_with("bun warning")
        || has_nonzero_summary_count(&lower, &["warning", "warnings"])
        || lower.contains(" warning ")
        || lower.contains(" warnings")
        || lower.contains("with warnings")
        || lower.contains("compiled with warning")
        || lower.contains("compiled with warnings")
        || lower.contains("warning ts")
        || lower.contains(": warning ts")
        || lower.contains(" - warning ts")
}

pub(super) fn is_diagnostic_block_start(line: &str) -> bool {
    is_typescript_diagnostic_line(line)
        || is_eslint_diagnostic_line(line)
        || is_junit_xml_failure_line(line)
        || is_exception_signal_line(line)
        || is_node_stack_error_line(line)
        || is_test_failure_signal_line(line)
        || is_stack_signal_line(line)
        || is_error_signal_line(line)
}

pub(super) fn is_diagnostic_detection_start(line: &str) -> bool {
    is_typescript_diagnostic_line(line)
        || is_eslint_diagnostic_line(line)
        || is_junit_xml_failure_line(line)
        || is_exception_signal_line(line)
        || is_node_stack_error_line(line)
        || is_test_failure_signal_line(line)
        || is_stack_signal_line(line)
        || line
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("npm err!")
}

pub(crate) fn is_typescript_diagnostic_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    (lower.contains("error ts")
        || lower.contains("warning ts")
        || lower.contains(" - error ts")
        || lower.contains(": error ts")
        || lower.contains(" - warning ts")
        || lower.contains(": warning ts"))
        && count_file_location_signals(trimmed) > 0
}

pub(crate) fn is_eslint_diagnostic_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    count_file_location_signals(trimmed) > 0
        && (lower.contains("  error  ")
            || lower.contains("  warning  ")
            || lower.contains(": error ")
            || lower.contains(": warning ")
            || lower.contains(" eslint "))
}

pub(crate) fn is_exception_signal_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with("E   ") {
        return true;
    }
    let Some((prefix, _)) = trimmed.split_once(':') else {
        return false;
    };
    let prefix = prefix.trim();
    if prefix.is_empty() || prefix.contains(' ') && !prefix.ends_with("Error") {
        return false;
    }
    matches!(
        prefix,
        "Error"
            | "AssertionError"
            | "ImportError"
            | "ModuleNotFoundError"
            | "NameError"
            | "RuntimeError"
            | "SyntaxError"
            | "TypeError"
            | "ValueError"
            | "ZeroDivisionError"
            | "ReferenceError"
            | "RangeError"
            | "URIError"
            | "EvalError"
    ) || prefix.ends_with("Error")
        || prefix.ends_with("Exception")
}
