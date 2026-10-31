use crate::session_root::{
    SessionResumeTarget, resolve_native_resume_target, resolve_session_root,
};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

const ROOT: &str = "01900000-0000-7000-8000-000000000710";
const CHILD: &str = "01900000-0000-7000-8000-000000000711";
const GRANDCHILD: &str = "01900000-0000-7000-8000-000000000712";
const OTHER: &str = "01900000-0000-7000-8000-000000000713";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(crate::tests::test_temp_dir("session-root"))
    }
    fn session(
        &self,
        id: &str,
        parent: Option<&str>,
        root: Option<&str>,
        version: &str,
    ) -> PathBuf {
        let path = self
            .0
            .join("sessions/2026/10/01")
            .join(format!("rollout-2026-10-01T01-00-00-{id}.jsonl"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source = parent.map_or(
            json!("cli"),
            |parent| json!({"subagent":{"thread_spawn":{"parent_thread_id":parent,"depth":1}}}),
        );
        fs::write(
            &path,
            format!(
                "{}\n",
                json!({"type":"session_meta","payload":{
                    "id":id, "session_id":root, "parent_thread_id":parent,
                    "source":source, "multi_agent_version":version
                }})
            ),
        )
        .unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn v2_child_and_nested_child_resolve_root_without_reading_large_history() {
    let f = Fixture::new();
    let root = f.session(ROOT, None, Some(ROOT), "v2");
    fs::OpenOptions::new()
        .write(true)
        .open(&root)
        .unwrap()
        .set_len(100 * 1024 * 1024)
        .unwrap();
    f.session(CHILD, Some(ROOT), Some(ROOT), "v2");
    f.session(GRANDCHILD, Some(CHILD), Some(ROOT), "v2");
    for id in [ROOT, CHILD, GRANDCHILD] {
        let actual = resolve_native_resume_target(&f.0, id).unwrap().unwrap();
        assert_eq!(
            actual,
            SessionResumeTarget {
                id: ROOT.to_owned(),
                path: root.clone()
            }
        );
        assert_eq!(resolve_session_root(&f.0, id).unwrap(), Some(actual));
    }
    assert_eq!(fs::metadata(root).unwrap().len(), 100 * 1024 * 1024);
}

#[test]
fn v1_direct_resume_and_independent_forks_are_not_retargeted() {
    let f = Fixture::new();
    f.session(ROOT, None, None, "v1");
    let child = f.session(CHILD, Some(ROOT), None, "v1");
    assert_eq!(
        resolve_native_resume_target(&f.0, CHILD).unwrap(),
        Some(SessionResumeTarget {
            id: CHILD.to_owned(),
            path: child
        })
    );
    assert_eq!(resolve_session_root(&f.0, CHILD).unwrap().unwrap().id, ROOT);
    let fork = f.session(OTHER, None, Some(OTHER), "v2");
    let mut header: serde_json::Value = serde_json::from_slice(&fs::read(&fork).unwrap()).unwrap();
    header["payload"]["forked_from_id"] = json!(ROOT);
    fs::write(&fork, format!("{header}\n")).unwrap();
    assert_eq!(
        resolve_native_resume_target(&f.0, OTHER)
            .unwrap()
            .unwrap()
            .id,
        OTHER
    );
}

#[test]
fn compressed_archived_parent_and_legacy_nested_source_are_supported() {
    let f = Fixture::new();
    let root = f.session(ROOT, None, None, "v1");
    let archive =
        f.0.join("archived_sessions")
            .join(format!("rollout-2026-10-01T01-00-00-{ROOT}.jsonl.zst"));
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    fs::write(
        &archive,
        zstd::stream::encode_all(fs::read(&root).unwrap().as_slice(), 1).unwrap(),
    )
    .unwrap();
    fs::remove_file(root).unwrap();
    let child = f.session(CHILD, Some(ROOT), Some(ROOT), "v2");
    let mut header: serde_json::Value = serde_json::from_slice(&fs::read(&child).unwrap()).unwrap();
    header["payload"]
        .as_object_mut()
        .unwrap()
        .remove("parent_thread_id");
    fs::write(child, format!("{header}\n")).unwrap();
    assert_eq!(
        resolve_native_resume_target(&f.0, CHILD).unwrap(),
        Some(SessionResumeTarget {
            id: ROOT.to_owned(),
            path: archive
        })
    );
}

#[test]
fn missing_input_never_uses_a_newest_unrelated_session() {
    let f = Fixture::new();
    f.session(OTHER, None, Some(OTHER), "v2");
    assert!(resolve_session_root(&f.0, CHILD).unwrap().is_none());
    assert!(resolve_session_root(&f.0, "01900000").unwrap().is_none());
}

#[test]
fn missing_cyclic_or_conflicting_ancestry_fails_without_rewriting_files() {
    let f = Fixture::new();
    let child = f.session(CHILD, Some(ROOT), Some(ROOT), "v2");
    let before = fs::read(&child).unwrap();
    assert!(
        resolve_native_resume_target(&f.0, CHILD)
            .unwrap_err()
            .to_string()
            .contains("missing")
    );
    assert_eq!(fs::read(&child).unwrap(), before);
    f.session(ROOT, Some(CHILD), Some(ROOT), "v2");
    assert!(
        resolve_session_root(&f.0, CHILD)
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );
    f.session(ROOT, None, Some(ROOT), "v2");
    f.session(CHILD, Some(ROOT), Some(OTHER), "v2");
    assert!(
        resolve_session_root(&f.0, CHILD)
            .unwrap_err()
            .to_string()
            .contains("recorded root")
    );
    f.session(CHILD, Some(CHILD), Some(CHILD), "v2");
    assert!(
        resolve_session_root(&f.0, CHILD)
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );
}

#[test]
fn malformed_or_disagreeing_parent_fields_are_not_accepted_as_roots() {
    let f = Fixture::new();
    f.session(ROOT, None, Some(ROOT), "v2");
    for parent in [json!(7), json!("not-a-uuid"), json!(OTHER)] {
        let child = f.session(CHILD, Some(ROOT), Some(ROOT), "v2");
        let mut header: serde_json::Value =
            serde_json::from_slice(&fs::read(&child).unwrap()).unwrap();
        header["payload"]["parent_thread_id"] = parent;
        fs::write(child, format!("{header}\n")).unwrap();
        assert!(resolve_session_root(&f.0, CHILD).is_err());
    }
}

#[test]
fn duplicate_root_rollouts_are_ambiguous_instead_of_first_match_wins() {
    let f = Fixture::new();
    let root = f.session(ROOT, None, Some(ROOT), "v2");
    f.session(CHILD, Some(ROOT), Some(ROOT), "v2");
    let duplicate = root.with_file_name(format!("rollout-2026-10-01T02-00-00-{ROOT}.jsonl"));
    fs::copy(root, duplicate).unwrap();
    assert!(
        resolve_session_root(&f.0, CHILD)
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
}

#[test]
fn malformed_input_prefix_is_left_for_existing_metadata_repair() {
    let f = Fixture::new();
    let child = f.session(CHILD, None, None, "v1");
    fs::write(&child, "not valid metadata\n").unwrap();
    assert!(resolve_native_resume_target(&f.0, CHILD).unwrap().is_none());
    assert_eq!(fs::read_to_string(&child).unwrap(), "not valid metadata\n");
}

#[test]
fn excessive_ancestry_depth_is_bounded() {
    let f = Fixture::new();
    for index in 0..=32 {
        let id = format!("01900000-0000-7000-8000-{index:012}");
        let parent = format!("01900000-0000-7000-8000-{:012}", index + 1);
        f.session(&id, Some(&parent), None, "v2");
    }
    assert!(
        resolve_session_root(&f.0, "01900000-0000-7000-8000-000000000000")
            .unwrap_err()
            .to_string()
            .contains("depth limit")
    );
}
