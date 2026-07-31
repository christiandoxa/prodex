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
fn context_audit_fail_fast_errors_escape_terminal_controls() {
    let root = temp_context_root("fail-fast-controls");
    let mut path = root.join("skills");
    for _ in 0..65 {
        path.push("nested\n\u{1b}[31m");
    }
    std::fs::create_dir_all(&path).expect("deep tree created");
    std::fs::write(path.join("SKILL.md"), "deep context").expect("deep file written");

    let error = collect_context_audit_report(&root, 20).expect_err("depth must be bounded");
    let message = format!("{error:#}");
    assert!(!message.contains('\n'));
    assert!(!message.contains('\u{1b}'));
    assert!(message.contains("\\n"));
    assert!(message.contains("\\u{1b}"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn context_audit_surfaces_bounded_read_errors_without_content() {
    let root = temp_context_root("read-errors");
    std::fs::create_dir_all(root.join("skills")).expect("skills directory created");
    for index in 0..65 {
        std::fs::write(
            root.join(format!("skills/broken-{index:02}.md")),
            b"secret-token \xff",
        )
        .expect("broken context written");
    }

    let report = collect_context_audit_report(&root, 20).expect("audit should complete");

    assert!(report.files.is_empty());
    assert_eq!(report.errors.len(), 64);
    assert_eq!(report.hidden_errors, 1);
    assert_eq!(report.errors[0].relative_path, "skills/broken-00.md");
    assert_eq!(report.errors[0].operation, "read");
    assert!(report.errors[0].message.contains("UTF-8"));

    let rendered = render_context_audit_report_with_width(&report, 20, 100);
    assert!(rendered.contains("Context Audit Errors"));
    assert!(rendered.contains("skills/broken-00.md"));
    assert!(rendered.contains("1 more audit errors hidden"));
    assert!(!rendered.contains("secret-token"));

    let json = serde_json::to_string(&report).expect("audit should serialize");
    assert!(json.contains("\"errors\""));
    assert!(!json.contains("secret-token"));

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn context_audit_keeps_readable_files_when_a_subtree_is_unreadable() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_context_root("traversal-errors");
    let blocked = root.join("skills/blocked");
    std::fs::create_dir_all(&blocked).expect("blocked directory created");
    std::fs::write(root.join("AGENTS.md"), "readable context").expect("context written");
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))
        .expect("permissions updated");
    if std::fs::read_dir(&blocked).is_ok() {
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700))
            .expect("permissions restored");
        let _ = std::fs::remove_dir_all(root);
        return;
    }

    let report = collect_context_audit_report(&root, 20).expect("audit should continue");

    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].relative_path, "AGENTS.md");
    assert!(report.errors.iter().any(|error| {
        error.operation == "traversal" && error.relative_path == "skills/blocked"
    }));

    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700))
        .expect("permissions restored");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn context_audit_escapes_terminal_controls_in_error_paths() {
    let root = temp_context_root("control-path");
    std::fs::create_dir_all(root.join("skills")).expect("skills directory created");
    std::fs::write(root.join("skills/broken\n\u{1b}[31m.md"), b"\xff")
        .expect("broken context written");

    let report = collect_context_audit_report(&root, 20).expect("audit should complete");

    assert_eq!(report.errors.len(), 1);
    assert_eq!(
        report.errors[0].relative_path,
        "skills/broken\\n\\u{1b}[31m.md"
    );
    let rendered = render_context_audit_report_with_width(&report, 20, 100);
    assert!(!rendered.contains("skills/broken\n"));
    assert!(!rendered.contains('\u{1b}'));

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
