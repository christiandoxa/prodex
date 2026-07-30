use super::*;
use std::os::unix::fs::{PermissionsExt as _, symlink};

#[test]
fn offline_self_test_accepts_system_temp_symlink_alias() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = fs::canonicalize(std::env::temp_dir()).unwrap();
    let target = temp_dir.join(format!(
        "prodex-smart-context-temp-target-{}-{unique}",
        std::process::id()
    ));
    let alias = temp_dir.join(format!(
        "prodex-smart-context-temp-alias-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&target).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&target, &alias).unwrap();

    let result = runtime_smart_context_self_test_persistence(&alias);

    assert!(
        fs::symlink_metadata(&alias)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let _ = fs::remove_file(&alias);
    let _ = fs::remove_dir_all(&target);
    result.expect("canonical system temp alias should be accepted");
}
