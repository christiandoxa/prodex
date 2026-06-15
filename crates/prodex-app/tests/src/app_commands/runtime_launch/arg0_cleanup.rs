use super::*;

#[test]
fn cleanup_codex_arg0_temp_dirs_removes_unlocked_stale_dirs() {
    let root = temp_dir("cleanup-arg0-stale");
    let codex_home = root.join("codex-home");
    let arg0_root = codex_home.join("tmp").join("arg0");
    let stale_dir = arg0_root.join("codex-arg0stale");
    let unrelated_dir = arg0_root.join("other-temp");
    fs::create_dir_all(&stale_dir).unwrap();
    fs::create_dir_all(&unrelated_dir).unwrap();
    fs::write(stale_dir.join(".lock"), "").unwrap();
    fs::write(stale_dir.join("apply_patch"), "").unwrap();

    cleanup_codex_arg0_temp_dirs_best_effort(&codex_home);

    assert!(!stale_dir.exists());
    assert!(unrelated_dir.exists());
}

#[test]
fn cleanup_codex_arg0_temp_dirs_skips_held_lock_dirs() {
    let root = temp_dir("cleanup-arg0-held-lock");
    let codex_home = root.join("codex-home");
    let held_dir = codex_home.join("tmp").join("arg0").join("codex-arg0held");
    fs::create_dir_all(&held_dir).unwrap();
    let lock_file = fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(held_dir.join(".lock"))
        .unwrap();
    lock_file.try_lock().unwrap();

    cleanup_codex_arg0_temp_dirs_best_effort(&codex_home);

    assert!(held_dir.exists());
}
