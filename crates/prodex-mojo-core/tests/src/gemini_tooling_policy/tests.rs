use super::*;

#[test]
fn gemini_tooling_policy_preserves_normalization_aliases_and_mutation() {
    let shell = gemini_tool_name_policy("  RUN-SHELL-COMMAND  ").unwrap();
    assert_eq!(shell.normalized, "run_shell_command");
    assert_eq!(shell.suffix, "run_shell_command");
    assert!(shell.aliases.contains(&"exec_command"));
    assert!(shell.aliases.contains(&"bash"));
    assert!(shell.mutating);
    assert!(shell.canonical_exec);

    let namespaced = gemini_tool_name_policy("mcp__repo__read-file").unwrap();
    assert_eq!(namespaced.normalized, "mcp__repo__read_file");
    assert_eq!(namespaced.suffix, "read_file");
    assert!(namespaced.aliases.is_empty());
    assert!(!namespaced.mutating);
    assert!(!namespaced.canonical_exec);

    let namespaced_exec = gemini_tool_name_policy("mcp__repo__exec-command").unwrap();
    assert_eq!(namespaced_exec.suffix, "exec_command");
    assert!(namespaced_exec.mutating);
    assert!(!namespaced_exec.canonical_exec);

    let dotted = gemini_tool_name_policy("namespace.Exec-Command").unwrap();
    assert_eq!(dotted.normalized, "exec_command");
    assert!(dotted.aliases.contains(&"run_shell_command"));
    assert!(dotted.mutating);

    let empty = gemini_tool_name_policy("   ").unwrap();
    assert_eq!(empty.normalized, "");
    assert_eq!(empty.suffix, "");
    assert!(empty.aliases.is_empty());
    assert!(!empty.mutating);
}

#[test]
fn gemini_unified_diff_kernel_converts_update_add_delete_and_move() {
    let input = concat!(
        "diff --git a/a.txt b/b.txt\n",
        "--- a/a.txt\n",
        "+++ b/b.txt\n",
        "@@ -1 +1 @@ context\n",
        "-old\n",
        "+new\n",
        "diff --git a/new.txt b/new.txt\n",
        "--- /dev/null\n",
        "+++ b/new.txt\n",
        "@@ -0,0 +1 @@\n",
        "+hello\n",
        "diff --git a/gone.txt b/gone.txt\n",
        "--- a/gone.txt\n",
        "+++ /dev/null\n",
    );
    assert_eq!(
        gemini_unified_diff_to_apply_patch(input).unwrap().unwrap(),
        concat!(
            "*** Begin Patch\n",
            "*** Update File: a.txt\n",
            "*** Move to: b.txt\n",
            "@@ context\n",
            "-old\n",
            "+new\n",
            "*** Add File: new.txt\n",
            "+hello\n",
            "*** Delete File: gone.txt\n",
            "*** End Patch",
        )
    );
    assert_eq!(
        gemini_unified_diff_to_apply_patch("plain text").unwrap(),
        None
    );
}

#[test]
fn gemini_apply_patch_block_kernel_normalizes_newlines_and_add_file_lines() {
    let input = "prefix\r\n  *** Begin Patch\r\n*** Add File: note.txt\r\nhello\r\n+world\r\n*** End Patch\r\ntrailer";
    assert_eq!(
        gemini_apply_patch_block(input).unwrap(),
        Some(
            "  *** Begin Patch\n*** Add File: note.txt\n+hello\n+world\n*** End Patch".to_string()
        )
    );
    assert_eq!(gemini_apply_patch_block("plain text").unwrap(), None);
}

#[test]
fn gemini_edit_patch_kernel_selects_add_update_and_no_match() {
    assert_eq!(
        gemini_edit_patch("note.txt", None, Some("hello\r\nworld")).unwrap(),
        Some("*** Begin Patch\n*** Add File: note.txt\n+hello\n+world\n*** End Patch".to_string())
    );
    assert_eq!(
        gemini_edit_patch("note.txt", Some("old"), Some("new")).unwrap(),
        Some(
            "*** Begin Patch\n*** Update File: note.txt\n@@\n-old\n+new\n*** End Patch".to_string()
        )
    );
    assert_eq!(
        gemini_edit_patch("note.txt", Some("old"), None).unwrap(),
        None
    );
    assert_eq!(gemini_edit_patch("  ", None, Some("new")).unwrap(), None);
}

#[test]
fn gemini_tooling_model_policy_preserves_gemini3_detection() {
    assert!(gemini_model_uses_gemini3_toolset("GEMINI-3-PRO").unwrap());
    assert!(gemini_model_uses_gemini3_toolset("auto").unwrap());
    assert!(gemini_model_uses_gemini3_toolset("AUTO-GEMINI-3").unwrap());
    assert!(!gemini_model_uses_gemini3_toolset("gemini-2.5-pro").unwrap());
}
