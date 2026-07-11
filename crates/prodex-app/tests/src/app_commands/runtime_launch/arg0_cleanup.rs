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

#[cfg(unix)]
#[test]
fn cleanup_codex_arg0_temp_dirs_skips_symlink_root() {
    let root = temp_dir("cleanup-arg0-symlink-root");
    let codex_home = root.join("codex-home");
    let arg0_parent = codex_home.join("tmp");
    let outside = root.join("outside-arg0");
    let outside_stale = outside.join("codex-arg0outside");
    fs::create_dir_all(&outside_stale).unwrap();
    fs::write(outside_stale.join(".lock"), "").unwrap();
    fs::create_dir_all(&arg0_parent).unwrap();
    std::os::unix::fs::symlink(&outside, arg0_parent.join("arg0")).unwrap();

    cleanup_codex_arg0_temp_dirs_best_effort(&codex_home);

    assert!(outside_stale.exists());
    assert_eq!(fs::read_to_string(outside_stale.join(".lock")).unwrap(), "");
}

#[cfg(unix)]
#[test]
fn cleanup_codex_arg0_temp_dirs_skips_symlink_child_dirs() {
    let root = temp_dir("cleanup-arg0-symlink-child");
    let codex_home = root.join("codex-home");
    let arg0_root = codex_home.join("tmp").join("arg0");
    let outside_stale = root.join("outside-stale");
    fs::create_dir_all(&arg0_root).unwrap();
    fs::create_dir_all(&outside_stale).unwrap();
    fs::write(outside_stale.join(".lock"), "").unwrap();
    fs::write(outside_stale.join("apply_patch"), "").unwrap();
    std::os::unix::fs::symlink(&outside_stale, arg0_root.join("codex-arg0linked")).unwrap();

    cleanup_codex_arg0_temp_dirs_best_effort(&codex_home);

    assert!(outside_stale.exists());
    assert!(outside_stale.join("apply_patch").exists());
    assert!(fs::symlink_metadata(arg0_root.join("codex-arg0linked")).is_ok());
}
