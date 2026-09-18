use super::*;

#[test]
fn rg_json_output_groups_matches_and_skips_metadata() {
    let input = "\
{\"type\":\"begin\",\"data\":{\"path\":{\"text\":\"src/lib.rs\"}}}
{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"src/lib.rs\"},\"lines\":{\"text\":\"fn alpha() {}\\n\"},\"line_number\":10}}
{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"README.md\"},\"lines\":{\"text\":\"prodex alpha\\n\"},\"line_number\":4}}
{\"type\":\"end\",\"data\":{\"path\":{\"text\":\"src/lib.rs\"}}}
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_search_matches_per_file: 2,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(report.output.contains("sum: search matches=2, files=2"));
    assert!(report.output.contains("src/lib.rs (1 matches):"));
    assert!(report.output.contains("10: fn alpha() {}"));
    assert!(report.output.contains("README.md (1 matches):"));
    assert!(!report.output.contains("\"type\":\"begin\""));
}

#[test]
fn rg_heading_output_uses_current_file_for_numbered_matches() {
    let input = "\
src/lib.rs
10:fn alpha() {}
20:fn beta() {}
--
README.md
3:prodex alpha
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::Search);
    assert!(report.output.contains("src/lib.rs (2 matches):"));
    assert!(report.output.contains("10: fn alpha() {}"));
    assert!(report.output.contains("README.md (1 matches):"));
}

#[test]
fn file_list_output_accepts_bare_paths_and_ls_listing_rows() {
    let input = "\
total 16
-rw-r--r-- 1 user group 10 May 1 12:00 Cargo.toml
-rw-r--r-- 1 user group 20 May 1 12:00 README.md
drwxr-xr-x 2 user group 4096 May 1 12:00 crates
src/main.rs
";

    let report = compact_command_output_with_options(
        input,
        &CommandOutputCompactOptions {
            kind: CommandOutputKind::Auto,
            max_path_entries: 10,
            ..CommandOutputCompactOptions::default()
        },
    );

    assert_eq!(report.detected_kind, CommandOutputKind::FileList);
    assert!(report.output.contains("sum: files entries=4"));
    assert!(report.output.contains("Cargo.toml"));
    assert!(report.output.contains("README.md"));
    assert!(report.output.contains("src/main.rs"));
}
