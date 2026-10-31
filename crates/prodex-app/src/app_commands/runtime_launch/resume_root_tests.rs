use super::normalize_super_resume_root_in_home;
use prodex_state::AppState;
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const ROOT: &str = "01900000-0000-7000-8000-000000000710";
const CHILD: &str = "01900000-0000-7000-8000-000000000711";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "prodex-resume-root-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let f = Self(root);
        f.session(ROOT, None, "v2");
        f.session(CHILD, Some(ROOT), "v2");
        f
    }
    fn session(&self, id: &str, parent: Option<&str>, version: &str) -> PathBuf {
        let path = self
            .0
            .join("sessions/2026/10/01")
            .join(format!("rollout-2026-10-01T01-00-00-{id}.jsonl"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source = parent.map_or(
            json!("cli"),
            |id| json!({"subagent":{"thread_spawn":{"parent_thread_id":id,"depth":1}}}),
        );
        fs::write(
            &path,
            format!(
                "{}\n",
                json!({"type":"session_meta","payload":{
                    "id":id,"session_id":ROOT,"parent_thread_id":parent,"source":source,
                    "multi_agent_version":version,"cwd":"/tmp/example-workspace"
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

fn arguments(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn direct_uuid_and_native_picker_select_the_same_root() {
    let f = Fixture::new();
    let mut direct = arguments(&[CHILD]);
    assert_eq!(
        normalize_super_resume_root_in_home(&f.0, &mut direct).unwrap(),
        Some(ROOT.to_owned())
    );
    assert_eq!(direct, arguments(&["resume", ROOT]));
    let reports =
        prodex_session_store::collect_session_reports(&f.0, None, &AppState::default()).unwrap();
    assert_eq!(
        reports.len(),
        1,
        "the picker must exclude parent-owned workers"
    );
    assert_eq!(reports[0].id, ROOT);
    let mut selected = arguments(&["resume", &reports[0].id]);
    assert_eq!(
        normalize_super_resume_root_in_home(&f.0, &mut selected).unwrap(),
        None
    );
    assert_eq!(selected, direct);
}

#[test]
fn root_retargeting_preserves_options_and_prompt_even_when_values_match_child_id() {
    let f = Fixture::new();
    let mut input = arguments(&[
        "--model",
        CHILD,
        "-c",
        r#"model_reasoning_effort="high""#,
        "resume",
        CHILD,
        CHILD,
        "--no-alt-screen",
    ]);
    let mut expected = input.clone();
    expected[5] = ROOT.into();
    assert_eq!(
        normalize_super_resume_root_in_home(&f.0, &mut input).unwrap(),
        Some(ROOT.to_owned())
    );
    assert_eq!(
        input, expected,
        "only the actual session argv slot may change"
    );
}

#[test]
fn roots_missing_sessions_remote_targets_and_exec_keep_native_arguments() {
    let f = Fixture::new();
    for values in [
        vec![ROOT],
        vec!["resume"],
        vec!["resume", "--last"],
        vec!["resume", "01900000-0000-7000-8000-000000000999"],
        vec!["--remote", "unix:///tmp/example.sock", "resume", CHILD],
        vec!["--remote=unix:///tmp/example.sock", "resume", CHILD],
        vec!["exec", "resume", CHILD],
    ] {
        let mut input = arguments(&values);
        let original = input.clone();
        assert_eq!(
            normalize_super_resume_root_in_home(&f.0, &mut input).unwrap(),
            None
        );
        assert_eq!(input, original);
    }
    f.session(CHILD, Some(ROOT), "v1");
    let mut input = arguments(&["resume", CHILD]);
    let original = input.clone();
    assert_eq!(
        normalize_super_resume_root_in_home(&f.0, &mut input).unwrap(),
        None
    );
    assert_eq!(input, original);
}

#[test]
fn missing_parent_stops_before_any_resume_repair_or_argument_change() {
    let f = Fixture::new();
    let root = f.session(ROOT, None, "v2");
    let child = f.session(CHILD, Some(ROOT), "v2");
    fs::remove_file(root).unwrap();
    let contents = fs::read(&child).unwrap();
    let mut input = arguments(&[CHILD]);
    assert!(normalize_super_resume_root_in_home(&f.0, &mut input).is_err());
    assert_eq!(input, arguments(&[CHILD]));
    assert_eq!(fs::read(child).unwrap(), contents);
}
