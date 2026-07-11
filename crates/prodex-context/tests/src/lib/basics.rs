use super::*;

#[test]
fn plain_command_output_strips_ansi_and_keeps_head_tail() {
    let input =
        "\u{1b}[31mline0\u{1b}[0m\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\n";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Plain,
        max_lines: 5,
        head_lines: 2,
        tail_lines: 2,
        max_line_chars: 80,
        ..CommandOutputCompactOptions::default()
    };

    let report = compact_command_output_with_options(input, &options);

    assert_eq!(report.detected_kind, CommandOutputKind::Plain);
    assert!(report.output.contains("line0"));
    assert!(report.output.contains("line1"));
    assert!(report.output.contains("line8"));
    assert!(report.output.contains("line9"));
    assert!(report.output.contains("[... omitted 6 lines ...]"));
    assert!(!report.output.contains("\u{1b}"));
}

#[test]
fn plain_command_output_uses_path_alias_for_repeated_absolute_cwd_prefix() {
    let cwd = test_cwd_prefix();
    let input = format!(
        "\
loaded {cwd}/src/main.rs
cached {cwd}/crates/prodex-context/src/lib.rs
"
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Plain,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Plain);
    assert!(
        report
            .output
            .contains(&format!("path aliases: $REPO={cwd}"))
    );
    assert!(report.output.contains("loaded $REPO/src/main.rs"));
    assert!(
        report
            .output
            .contains("cached $REPO/crates/prodex-context/src/lib.rs")
    );
}

#[test]
fn compact_command_output_structured_json_array_summarizes_shape_and_errors() {
    let mut input = String::from("[\n");
    for index in 0..80 {
        input.push_str(&format!(
            "  {{\"id\":\"req-{index}\",\"path\":\"module/{index}\",\"result\":\"ok {index}\"}},\n"
        ));
    }
    input.push_str(
        "  {\"id\":\"req-error\",\"path\":\"src/lib.rs:42\",\"error\":{\"code\":\"E42\",\"message\":\"failed to parse src/lib.rs:42\"}}\n]",
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 40,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert!(report.output.contains("pcs: json"));
    assert!(report.output.contains("shape: array items=81"));
    assert!(report.output.contains("error samples:"));
    assert!(report.output.contains("E42"));
    assert!(report.estimated_tokens_after < report.estimated_tokens_before);
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn compact_command_output_ndjson_summarizes_records_and_errors() {
    let mut input = String::new();
    for index in 0..60 {
        input.push_str(&format!(
            "{{\"id\":\"evt-{index}\",\"path\":\"module-{index}\",\"event\":\"processed\"}}\n"
        ));
    }
    input.push_str(
        "{\"id\":\"evt-error\",\"path\":\"src/lib.rs:42\",\"error\":{\"code\":\"E_IO\",\"message\":\"failed src/lib.rs:42\"}}\n",
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 40,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert!(report.output.contains("pcs: ndjson"));
    assert!(report.output.contains("records=61"));
    assert!(report.output.contains("keys:"));
    assert!(report.output.contains("E_IO"));
    assert!(report.estimated_tokens_after < report.estimated_tokens_before);
    assert_no_critical_signal_loss(&input, &report.output);
}

#[test]
fn compact_command_output_non_json_falls_back_to_existing_plain_compaction() {
    let input = "plain line 1\nplain line 2\nplain line 3\n";
    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Plain,
            max_lines: 2,
            head_lines: 1,
            tail_lines: 1,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert!(!report.output.contains("pcs: json"));
    assert!(!report.output.contains("pcs: ndjson"));
    assert!(report.output.contains("plain line 1"));
    assert!(report.output.contains("plain line 3"));
}

#[test]
fn command_metadata_infers_output_kind_hints() {
    let cases = [
        (
            "{\"cmd\":\"cargo test -q\"}",
            CommandOutputKind::RustDiagnostics,
        ),
        (
            "command: cargo +nightly check --workspace",
            CommandOutputKind::RustDiagnostics,
        ),
        ("rg --json needle crates", CommandOutputKind::Search),
        ("grep -R needle src", CommandOutputKind::Search),
        ("git -C repo status --short", CommandOutputKind::GitStatus),
        ("git diff --stat", CommandOutputKind::GitDiff),
        ("git log --stat --oneline", CommandOutputKind::GitLog),
        ("pytest tests -q", CommandOutputKind::Diagnostics),
        ("python -m pytest tests", CommandOutputKind::Diagnostics),
        ("ruff check .", CommandOutputKind::Diagnostics),
        ("mypy src", CommandOutputKind::Diagnostics),
        ("biome check --write .", CommandOutputKind::Diagnostics),
        ("oxlint --fix", CommandOutputKind::Diagnostics),
        ("npx tsc --noEmit", CommandOutputKind::Diagnostics),
        (
            "cargo clippy --fix --allow-dirty",
            CommandOutputKind::RustDiagnostics,
        ),
        ("cargo fmt --all", CommandOutputKind::RustDiagnostics),
        ("npm test -- --runInBand", CommandOutputKind::Diagnostics),
        (
            "npm --prefix web run typecheck",
            CommandOutputKind::Diagnostics,
        ),
        (
            "uv pip install -r requirements.txt",
            CommandOutputKind::NoisySuccess,
        ),
        ("bazel test //...", CommandOutputKind::NoisySuccess),
        ("npx nx affected -t build", CommandOutputKind::NoisySuccess),
        ("turbo run build", CommandOutputKind::NoisySuccess),
        ("docker compose up --wait", CommandOutputKind::NoisySuccess),
        ("kubectl logs deploy/prodex", CommandOutputKind::LogStream),
        ("ls -la crates", CommandOutputKind::FileList),
        (
            "find crates -maxdepth 2 -type f",
            CommandOutputKind::FileList,
        ),
        ("tree -L 2 crates", CommandOutputKind::FileList),
    ];

    for (metadata, expected) in cases {
        assert_eq!(
            infer_command_output_kind_from_metadata(metadata),
            Some(expected),
            "metadata: {metadata}"
        );
    }
}

#[test]
fn command_metadata_hint_compacts_single_search_match_as_search() {
    let input = "src/lib.rs:42:needle once\n";
    let hint = infer_command_output_kind_from_metadata("rg needle src");
    let report = compact_command_output_with_options_and_kind_hint(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
        hint,
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(report.output.contains("sum: search matches=1, files=1"));
    assert!(report.output.contains("src/lib.rs (1 matches):"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn command_metadata_hint_compacts_quiet_cargo_output_as_rust_diagnostics() {
    let input = "Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s\n";
    let hint = infer_command_output_kind_from_metadata("cargo check --workspace");
    let report = compact_command_output_with_options_and_kind_hint(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
        hint,
    );

    assert_eq!(report.detected_kind, CommandOutputKind::RustDiagnostics);
    assert!(report.output.contains("sum: rust"));
    assert!(report.output.contains("Finished `dev` profile"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn command_metadata_hint_preserves_new_tool_warning_summaries() {
    let input = "\
Found 1 warning and 0 errors.
Finished in 12ms on 42 files with 1 warning.
";
    let hint = infer_command_output_kind_from_metadata("oxlint .");
    let report = compact_command_output_with_options_and_kind_hint(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
        hint,
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("Found 1 warning and 0 errors."));
    assert!(
        report
            .output
            .contains("Finished in 12ms on 42 files with 1 warning.")
    );
    assert!(!report.output.contains("noise:"));
    assert_no_critical_signal_loss(input, &report.output);
}

#[test]
fn command_metadata_hint_does_not_override_strong_output_detection() {
    let input = "\
src/app.ts:12:7 - error TS2322: Type 'string' is not assignable to type 'number'.

12 const count: number = value;
        ~~~~~
";
    let report = compact_command_output_with_options_and_kind_hint(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_lines: 20,
            ..CommandOutputCompactOptions::default()
        },
        Some(CommandOutputKind::Search),
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Diagnostics);
    assert!(report.output.contains("error TS2322"));
    assert_no_critical_signal_loss(input, &report.output);
}
