use super::*;

#[cfg(unix)]
#[test]
fn context_walk_and_compress_skip_symlinks() {
    use std::os::unix::fs::symlink;

    let root = temp_context_root("symlink");
    std::fs::create_dir_all(root.join("skills")).expect("skills created");
    let outside = root.with_file_name("prodex-context-outside.md");
    let outside_dir = root.with_file_name("prodex-context-outside-dir");
    std::fs::create_dir_all(&outside_dir).expect("outside directory created");
    std::fs::write(&outside, "outside secret context").expect("outside file written");
    std::fs::write(outside_dir.join("SKILL.md"), "outside directory context")
        .expect("outside directory file written");
    symlink(&outside, root.join("skills/escape.md")).expect("symlink created");
    symlink(&outside_dir, root.join("skills/escape-dir")).expect("directory symlink created");

    let audit = collect_context_audit_report(&root, 20).expect("audit succeeds");
    assert!(audit.files.is_empty());
    let compressed = compress_context_path(&root.join("skills"), false).expect("compress succeeds");
    assert!(compressed.entries.is_empty());
    assert_eq!(
        std::fs::read_to_string(&outside).expect("outside file readable"),
        "outside secret context"
    );

    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_file(outside);
    let _ = std::fs::remove_dir_all(outside_dir);
}

#[test]
fn context_walk_rejects_excessive_depth() {
    let root = temp_context_root("depth");
    let mut path = root.join("skills");
    for _ in 0..65 {
        path.push("nested");
    }
    std::fs::create_dir_all(&path).expect("deep tree created");
    std::fs::write(path.join("SKILL.md"), "deep context").expect("deep file written");

    let error = collect_context_audit_report(&root, 20).expect_err("depth must be bounded");
    assert!(error.to_string().contains("depth exceeded"));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn context_compress_preserves_private_mode() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let root = temp_context_root("mode");
    let path = root.join("AGENTS.md");
    std::fs::write(
        &path,
        "This is actually a very verbose paragraph in order to make sure to reduce tokens.\n",
    )
    .expect("context written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("mode set");

    compress_context_path(&path, false).expect("compress succeeds");
    assert_eq!(
        std::fs::metadata(&path).expect("metadata").mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(root.join("AGENTS.original.md"))
            .expect("backup metadata")
            .mode()
            & 0o777,
        0o600
    );
    let _ = std::fs::remove_dir_all(root);
}
