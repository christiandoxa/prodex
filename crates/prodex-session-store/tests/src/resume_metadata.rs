use super::*;
use std::io::{Seek, SeekFrom};

const ID: &str = "01900000-0000-7000-8000-000000000810";

fn header(id: &str) -> String {
    format!(
        "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"cwd\":\"/home/test-user/project\",\"model_provider\":\"openai\",\"originator\":\"codex-cli\",\"cli_version\":\"0.167.0\"}}}}\n"
    )
}

#[test]
fn launch_metadata_ignores_large_body_and_reads_latest_complete_turn_context() {
    let root = test_temp_dir("bounded-resume-metadata");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    let path = sessions.join(format!("rollout-{ID}.jsonl"));
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(header(ID).as_bytes()).unwrap();
    // A sparse oversized opaque interior proves startup never parses the transcript.
    file.seek(SeekFrom::Start(4 * 1024 * 1024)).unwrap();
    file.write_all(b"\n{\"type\":\"turn_context\",\"payload\":{\"model\":\"fixture-model\",\"reasoning_effort\":\"high\",\"cwd\":\"/home/test-user/current\"}}\n").unwrap();
    // A partially appended final line must not overwrite the last complete record.
    file.write_all(b"{\"type\":\"turn_context\",\"payload\":{\"model\":\"partial")
        .unwrap();
    drop(file);
    let report = resolve_resume_launch_report(&root, &AppState::default(), ID).unwrap();
    assert_eq!(report.id, ID);
    assert_eq!(report.model_provider.as_deref(), Some("openai"));
    assert_eq!(report.last_model(), Some("fixture-model"));
    assert_eq!(report.last_reasoning_effort(), Some("high"));
    assert_eq!(report.cwd.as_deref(), Some("/home/test-user/current"));
    assert_eq!(
        session_cwd_from_path(&path).unwrap(),
        Some(PathBuf::from("/home/test-user/project"))
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn launch_metadata_keeps_native_restoration_when_tail_has_no_settings() {
    let root = test_temp_dir("bounded-resume-no-settings");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    let path = sessions.join(format!("rollout-{ID}.jsonl"));
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(header(ID).as_bytes()).unwrap();
    file.write_all(b"{\"type\":\"turn_context\",\"payload\":{\"model\":\"obsolete\"}}\n")
        .unwrap();
    file.seek(SeekFrom::Start(4 * 1024 * 1024)).unwrap();
    file.write_all(b"\n").unwrap();
    drop(file);
    let report = resolve_resume_launch_report(&root, &AppState::default(), ID).unwrap();
    assert_eq!(report.last_model(), None);
    assert_eq!(report.model_provider.as_deref(), Some("openai"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn launch_metadata_keeps_duplicate_identity_ambiguity() {
    let root = test_temp_dir("bounded-resume-ambiguity");
    for directory in ["sessions", "archived_sessions"] {
        fs::create_dir_all(root.join(directory)).unwrap();
        fs::write(
            root.join(directory).join(format!("rollout-{ID}.jsonl")),
            header(ID),
        )
        .unwrap();
    }
    assert!(matches!(
        resolve_resume_launch_report(&root, &AppState::default(), ID),
        Err(SessionResolveError::Ambiguous { .. })
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_exact_launch_target_ignores_unreadable_unrelated_header() {
    let root = test_temp_dir("bounded-resume-missing");
    fs::create_dir_all(root.join("sessions")).unwrap();
    fs::write(root.join("sessions/unrelated.jsonl"), [255_u8; 1024]).unwrap();
    assert!(matches!(
        resolve_resume_launch_report(&root, &AppState::default(), ID),
        Err(SessionResolveError::Missing { .. })
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn launch_metadata_keeps_unindexed_generic_names_and_complete_unterminated_json() {
    let root = test_temp_dir("bounded-resume-legacy-name");
    fs::create_dir_all(root.join("sessions")).unwrap();
    let raw = format!(
        "{}{{\"type\":\"turn_context\",\"payload\":{{\"model\":\"fixture-model\",\"effort\":\"max\"}}}}",
        header(ID)
    );
    fs::write(root.join("sessions/rollout.jsonl"), raw).unwrap();
    let report = resolve_resume_launch_report(&root, &AppState::default(), ID).unwrap();
    assert_eq!(report.id, ID);
    assert_eq!(report.last_model(), Some("fixture-model"));
    assert_eq!(report.last_reasoning_effort(), Some("max"));
    fs::remove_dir_all(root).unwrap();
}
