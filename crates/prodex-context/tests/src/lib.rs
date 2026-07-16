use super::*;

fn assert_no_critical_signal_loss(before: &str, after: &str) {
    let check = critical_signal_self_check(before, after);
    assert!(
        check.passed(),
        "lost critical signals: {:?}\ncompacted output:\n{}",
        check.lost,
        after
    );
}

fn assert_token_regression_budget(
    before: &str,
    report: &CommandOutputCompactReport,
    min_saved_percent: usize,
) {
    assert_no_critical_signal_loss(before, &report.output);
    assert!(
        report.estimated_tokens_after < report.estimated_tokens_before,
        "compaction did not save tokens: before={}, after={}\n{}",
        report.estimated_tokens_before,
        report.estimated_tokens_after,
        report.output
    );
    let saved = report
        .estimated_tokens_before
        .saturating_sub(report.estimated_tokens_after);
    assert!(
        saved.saturating_mul(100)
            >= report
                .estimated_tokens_before
                .saturating_mul(min_saved_percent),
        "saved {saved}/{} tokens, expected at least {min_saved_percent}%\n{}",
        report.estimated_tokens_before,
        report.output
    );
}

fn test_cwd_prefix() -> String {
    std::env::current_dir()
        .expect("test cwd")
        .display()
        .to_string()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}

fn temp_context_root(name: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "prodex-context-{name}-{}-{unique}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp context root should be created");
    root
}

#[path = "lib/basics.rs"]
mod basics;

#[path = "lib/git_search.rs"]
mod git_search;

#[path = "lib/intent.rs"]
mod intent;

#[path = "lib/diagnostics.rs"]
mod diagnostics;

#[path = "lib/diagnostic_extra.rs"]
mod diagnostic_extra;

#[path = "lib/git_noisy.rs"]
mod git_noisy;

#[path = "lib/success_summary.rs"]
mod success_summary;

#[path = "lib/success_summary_tools.rs"]
mod success_summary_tools;

#[path = "lib/search_log.rs"]
mod search_log;

#[path = "lib/critical_blob.rs"]
mod critical_blob;

#[path = "lib/context_fs.rs"]
mod context_fs;
