use super::*;
use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};

#[test]
fn gemini_masked_tool_output_creates_private_directory_and_file() {
    let root = runtime_gemini_tool_output_test_path("permissions");
    let directory = root.join("nested");
    let path = directory.join("output.txt");

    runtime_gemini_write_masked_tool_output(&path, "sensitive output").unwrap();

    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(fs::read_to_string(path).unwrap(), "sensitive output");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_masked_tool_output_rejects_symlink_parent() {
    let root = runtime_gemini_tool_output_test_path("symlink-parent");
    let trusted = root.join("trusted");
    let outside = root.join("outside");
    fs::create_dir_all(&trusted).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&trusted, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&outside, trusted.join("linked")).unwrap();

    let result = runtime_gemini_write_masked_tool_output(
        &trusted.join("linked/output.txt"),
        "sensitive output",
    );

    assert!(result.is_err());
    assert!(!outside.join("output.txt").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn gemini_masked_tool_output_does_not_follow_final_symlink() {
    let root = runtime_gemini_tool_output_test_path("symlink-final");
    fs::create_dir_all(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let outside = root.join("outside.txt");
    let path = root.join("output.txt");
    fs::write(&outside, "outside").unwrap();
    symlink(&outside, &path).unwrap();

    runtime_gemini_write_masked_tool_output(&path, "sensitive output").unwrap();

    assert_eq!(fs::read_to_string(&outside).unwrap(), "outside");
    assert_eq!(fs::read_to_string(&path).unwrap(), "sensitive output");
    assert!(fs::symlink_metadata(&path).unwrap().is_file());
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::remove_dir_all(root).unwrap();
}

fn runtime_gemini_tool_output_test_path(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!(
        "prodex-gemini-tool-output-{name}-{}-{nanos}",
        std::process::id()
    ))
}
