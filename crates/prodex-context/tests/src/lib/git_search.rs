use super::*;

#[test]
fn git_status_short_output_groups_status_categories() {
    let input = "\
## main...origin/main
 M README.md
M  crates/prodex-context/src/lib.rs
R  old.txt -> new.txt
?? notes.txt
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitStatus);
    assert!(report.output.contains("branch: main...origin/main"));
    assert!(
        report
            .output
            .contains("staged (1): M crates/prodex-context/src/lib.rs")
    );
    assert!(report.output.contains("modified (1): M README.md"));
    assert!(report.output.contains("renamed (1): R old.txt -> new.txt"));
    assert!(report.output.contains("untracked (1): notes.txt"));
}

#[test]
fn git_status_output_shortens_repeated_absolute_cwd_prefix() {
    let cwd = test_cwd_prefix();
    let input = format!(
        "\
## main...origin/main
 M {cwd}/src/lib.rs
?? {cwd}/tests/new.rs
"
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::GitStatus);
    assert!(
        report
            .output
            .contains(&format!("path aliases: $REPO={cwd}"))
    );
    assert!(report.output.contains("modified (1): M $REPO/src/lib.rs"));
    assert!(report.output.contains("untracked (1): $REPO/tests/new.rs"));
}

#[test]
fn git_diff_output_keeps_summary_and_hunk_markers() {
    let input = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,12 @@
-old
+new0
+new1
+new2
+new3
+new4
+new5
+new6
+new7
+new8
+new9
 context
";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Auto,
        max_lines: 20,
        head_lines: 10,
        tail_lines: 3,
        max_line_chars: 120,
        ..CommandOutputCompactOptions::default()
    };

    let report = compact_command_output_with_options(input, &options);

    assert_eq!(report.detected_kind, CommandOutputKind::GitDiff);
    assert!(
        report
            .output
            .contains("sum: git diff files=1, +10, -1, hunks=1")
    );
    assert!(report.output.contains("src/lib.rs: +10, -1, 1 hunks"));
    assert!(report.output.contains("@@ -1,3 +1,12 @@"));
    assert!(report.output.contains("omitted"));
}

#[test]
fn search_output_groups_matches_by_file() {
    let input = "\
src/lib.rs:10:fn alpha() {}
src/lib.rs:20:5:let beta = true;
README.md:3:prodex context helper
";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Auto,
        max_search_matches_per_file: 1,
        ..CommandOutputCompactOptions::default()
    };

    let report = compact_command_output_with_options(input, &options);

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(report.output.contains("sum: search matches=3, files=2"));
    assert!(report.output.contains("README.md (1 matches):"));
    assert!(report.output.contains("src/lib.rs (2 matches):"));
    assert!(
        report
            .output
            .contains("[... 1 more matches in this file ...]")
    );
}

#[test]
fn search_output_shortens_repeated_absolute_cwd_prefix() {
    let cwd = test_cwd_prefix();
    let input = format!(
        "\
{cwd}/src/lib.rs:10:fn alpha() {{}}
{cwd}/src/lib.rs:20:fn beta() {{}}
{cwd}/README.md:3:prodex context helper
"
    );

    let report = compact_command_output_with_options(
        &input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_search_matches_per_file: 2,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(
        report
            .output
            .contains(&format!("path aliases: $REPO={cwd}"))
    );
    assert!(report.output.contains("$REPO/src/lib.rs (2 matches):"));
    assert!(report.output.contains("$REPO/README.md (1 matches):"));
}

#[test]
fn search_with_many_rust_file_matches_stays_search_output() {
    let input = "\
src/lib.rs:10:fn alpha() {}
src/lib.rs:20:fn beta() {}
src/app.rs:30:fn gamma() {}
crates/prodex-context/src/lib.rs:40:fn delta() {}
crates/prodex-context/tests/src/lib.rs:50:fn epsilon() {}
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
}

#[test]
fn file_list_output_summarizes_and_truncates_entries() {
    let input = "\
./src/main.rs
./src/lib.rs
./src/app_commands/context.rs
./crates/prodex-context/src/lib.rs
./crates/prodex-context/Cargo.toml
./README.md
./docs/testing.md
./target/debug/prodex
";
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::Auto,
        max_path_entries: 4,
        ..CommandOutputCompactOptions::default()
    };

    let report = compact_command_output_with_options(input, &options);

    assert_eq!(report.detected_kind, CommandOutputKind::FileList);
    assert!(report.output.contains("sum: files entries=8"));
    assert!(report.output.contains("top roots: src=3"));
    assert!(report.output.contains("extensions: rs=4"));
    assert!(
        report
            .output
            .contains("[... omitted 4 file-list entries ...]")
    );
}

#[test]
fn explicit_kind_overrides_auto_detection() {
    let input = "src/lib.rs:10:fn alpha() {}\n";
    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Plain,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.requested_kind, CommandOutputKind::Plain);
    assert_eq!(report.detected_kind, CommandOutputKind::Plain);
    assert_eq!(report.output, input);
}
