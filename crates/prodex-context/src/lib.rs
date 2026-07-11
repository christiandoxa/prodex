use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use terminal_ui::section_header;

mod audit;
mod blob_noise;
mod command_output;
#[path = "lib/compression.rs"]
mod compression;
mod critical_signal;
pub use audit::{
    ContextAuditEntry, ContextAuditReport, ContextCompressEntry, ContextCompressReport,
    ContextStaticDuplicateOccurrence, ContextStaticDuplicateReport, ContextStaticDuplicateSnippet,
    collect_context_audit_report, collect_context_static_duplicate_report,
    render_context_audit_report_with_width,
};
pub(crate) use audit::{collect_context_files, format_count};
pub use blob_noise::{
    ContextBlobNoiseFinding, ContextBlobNoiseKind, ContextBlobNoiseReport,
    detect_context_blob_noise, detect_context_blob_noise_for_path, is_context_blob_noise,
};
#[cfg(test)]
pub(crate) use command_output::is_generated_compaction_header_line;
pub use command_output::{
    CommandOutputCompactLimits, CommandOutputCompactOptions, CommandOutputCompactReport,
    CommandOutputIntentCompactOptions, CommandOutputKind, CommandSuccessOutputCompactOptions,
    CommandSuccessOutputCompactReport, MAX_EXTRACTED_INTENT_TERMS,
    command_output_kind_hint_for_command, compact_command_output,
    compact_command_output_with_intent_options, compact_command_output_with_intent_terms,
    compact_command_output_with_options, compact_command_output_with_options_and_kind_hint,
    compact_successful_command_output_with_options, extract_intent_terms_from_prompt,
    infer_command_output_kind_from_metadata,
};
pub(crate) use command_output::{
    command_lines, count_text_lines, generic_failed_test_name, has_zero_only_summary_count,
    is_eslint_diagnostic_line, is_exception_signal_line, is_junit_xml_failure_line,
    is_log_level_signal_line, is_rust_backtrace_start, is_rust_exit_status_line,
    is_rust_failure_summary_line, is_rust_panic_line, is_typescript_diagnostic_line,
    normalize_command_output, parse_file_list_entry_line, parse_rg_json_match_line,
    parse_search_match_line, rust_diagnostic_severity, rust_failed_test_name,
    rust_failure_separator_name,
};
pub use compression::{
    compress_context_path, compress_context_text, render_context_compress_report,
};
pub(crate) use compression::{
    estimate_context_tokens, is_compressible_context_file, is_context_backup,
};
pub use critical_signal::{
    CriticalSignalCounts, CriticalSignalLineRange, CriticalSignalLineRangeOptions,
    CriticalSignalSelfCheck, count_critical_signals, critical_signal_lost_line_ranges,
    critical_signal_lost_line_ranges_with_options, critical_signal_self_check,
};

#[cfg(test)]
mod success_short_form_tests {
    use super::*;

    fn success_options(command: &str) -> CommandSuccessOutputCompactOptions {
        CommandSuccessOutputCompactOptions {
            command: Some(command.to_string()),
            exit_code: Some(0),
            min_lines_to_compact: 1,
            max_touched_files: 4,
            max_key_lines: 4,
            max_line_chars: 160,
        }
    }

    fn assert_short_success(report: &CommandSuccessOutputCompactReport, command: &str) {
        assert!(report.compacted, "{command}");
        assert!(!report.failure_suspected, "{command}");
        assert_eq!(report.critical_signals.total(), 0, "{command}");
        assert!(
            report.output.starts_with("pcs: ok "),
            "{command}: {}",
            report.output
        );
        assert!(report.output.contains("cmd:"), "{command}");
    }

    #[test]
    fn successful_command_output_short_forms_cargo_clippy_and_doc_noise() {
        let mut clippy = String::new();
        for index in 0..12 {
            clippy.push_str(&format!("    Checking crate_{index} v0.1.0\n"));
        }
        clippy.push_str("    Finished `dev` profile [unoptimized] target(s) in 1.23s\n");

        let report = compact_successful_command_output_with_options(
            &clippy,
            &success_options("cargo clippy --workspace --all-targets"),
        );
        assert_short_success(&report, "cargo clippy");
        assert!(report.output.contains("checking=12"));
        assert!(!report.output.contains("Checking crate_0"));

        let mut docs = String::new();
        for index in 0..12 {
            docs.push_str(&format!(" Documenting crate_{index} v0.1.0\n"));
        }
        docs.push_str("    Finished `dev` profile [unoptimized] target(s) in 2.34s\n");
        docs.push_str("   Generated /repo/target/doc/prodex/index.html\n");

        let report =
            compact_successful_command_output_with_options(&docs, &success_options("cargo doc"));
        assert_short_success(&report, "cargo doc");
        assert!(report.touched_files > 0);
        assert!(report.output.contains("documenting=12"));
        assert!(!report.output.contains("/repo/target/doc/prodex/index.html"));
    }

    #[test]
    fn successful_command_output_short_forms_common_frontend_success_noise() {
        let tsc = "\
Projects in this build:
    * tsconfig.json
Project 'tsconfig.json' is up to date because newest input 'src/app.ts' is older than output 'tsconfig.tsbuildinfo'
Found 0 errors.
";
        let vite = "\
vite v5.4.19 building for production...
transforming...
✓ 42 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                  0.45 kB │ gzip: 0.29 kB
dist/assets/index-abc.js        24.12 kB │ gzip: 8.00 kB
✓ built in 1.23s
";
        let next = "\
▲ Next.js 15.3.1
Creating an optimized production build ...
✓ Compiled successfully in 1000ms
Linting and checking validity of types ...
Collecting page data ...
Generating static pages (0/5) ...
✓ Generating static pages (5/5)
Finalizing page optimization ...
Collecting build traces ...
Route (app)                              Size     First Load JS
┌ ○ /                                 5.56 kB         105 kB
+ First Load JS shared by all          99.6 kB
";
        let playwright = "\
Running 3 tests using 2 workers
✓ home page renders (120ms)
✓ settings page renders (130ms)
✓ login page renders (140ms)
3 passed (1.2s)
";
        let cypress = "\
Spec                                              Tests  Passing  Failing  Pending  Skipped
tests/app.cy.ts                                      4        4        0        0        0
Passing: 4
Failing: 0
All specs passed!
";
        let jest = "\
PASS tests/unit_0.test.ts
PASS tests/unit_1.test.ts
PASS tests/unit_2.test.ts
Test Suites: 3 passed, 3 total
Tests:       18 passed, 18 total
Snapshots:   0 total
Time:        1.23 s
Ran all test suites.
";

        for (command, input, omitted_line) in [
            ("npx tsc --noEmit", tsc, "Project 'tsconfig.json'"),
            ("vite build", vite, "dist/assets/index-abc.js"),
            ("next build", next, "Route (app)"),
            ("playwright test", playwright, "home page renders"),
            ("cypress run", cypress, "tests/app.cy.ts"),
            ("jest --runInBand", jest, "PASS tests/unit_0.test.ts"),
        ] {
            let report =
                compact_successful_command_output_with_options(input, &success_options(command));
            assert_short_success(&report, command);
            assert!(!report.output.contains(omitted_line), "{command}");
        }
    }

    #[test]
    fn successful_command_output_short_form_refuses_common_tool_warnings_and_failures() {
        for (command, input) in [
            (
                "next build",
                "Compiled with warnings\napp/page.tsx imports a large dependency\n",
            ),
            (
                "cypress run",
                "Spec Tests Passing Failing Pending Skipped\nFailing: 1\n",
            ),
            (
                "next build",
                "Type error: Page \"app/page.tsx\" has an invalid default export\n",
            ),
        ] {
            let report =
                compact_successful_command_output_with_options(input, &success_options(command));
            assert!(!report.compacted, "{command}");
            assert!(report.failure_suspected, "{command}");
            assert_eq!(report.output, input, "{command}");
        }
    }
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
