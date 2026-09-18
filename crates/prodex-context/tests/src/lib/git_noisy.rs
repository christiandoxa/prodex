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
