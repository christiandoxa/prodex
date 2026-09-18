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

fn temp_context_root(name: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temp dir should resolve")
        .join(format!(
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

#[path = "lib/git_noisy.rs"]
mod git_noisy;

#[path = "lib/search_log.rs"]
mod search_log;

#[path = "lib/critical_blob.rs"]
mod critical_blob;

#[path = "lib/context_fs.rs"]
mod context_fs;
