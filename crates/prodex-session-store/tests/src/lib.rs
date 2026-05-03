use super::*;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn session_reports_parse_codex_jsonl_metadata() {
    let root = test_temp_dir("session-jsonl");
    let sessions = root.join("sessions/2026/04/29");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    let cwd = root.join("workspace");
    fs::create_dir_all(&cwd).expect("workspace should be created");
    fs::write(
            sessions.join("session-a.jsonl"),
            format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"sess-a\",\"thread_name\":\"Issue triage\",\"cwd\":\"{}\"}}}}\n{{\"timestamp\":\"2026-04-29T12:30:00Z\",\"type\":\"event\"}}\n",
                cwd.display()
            ),
        )
        .expect("session should be written");

    let reports =
        collect_session_reports(&root, None, &AppState::default()).expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "sess-a");
    assert_eq!(reports[0].thread_name.as_deref(), Some("Issue triage"));
    assert_eq!(
        reports[0].cwd.as_deref(),
        Some(cwd.to_string_lossy().as_ref())
    );
    assert_eq!(
        reports[0].updated_at.as_deref(),
        Some("2026-04-29T12:30:00Z")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_current_filters_by_cwd() {
    let root = test_temp_dir("session-current");
    let sessions = root.join("sessions");
    let current = root.join("current");
    let other = root.join("other");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::create_dir_all(&current).expect("current dir should be created");
    fs::create_dir_all(&other).expect("other dir should be created");
    fs::write(
            sessions.join("current.jsonl"),
            format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"current\",\"cwd\":\"{}\"}}}}\n",
                current.display()
            ),
        )
        .expect("current session should be written");
    fs::write(
            sessions.join("other.jsonl"),
            format!(
                "{{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{{\"id\":\"other\",\"cwd\":\"{}\"}}}}\n",
                other.display()
            ),
        )
        .expect("other session should be written");

    let reports = collect_session_reports(&root, Some(&current), &AppState::default())
        .expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "current");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn session_reports_attach_profile_bindings() {
    let root = test_temp_dir("session-profile");
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("session dir should be created");
    fs::write(
        sessions.join("bound.jsonl"),
        "{\"timestamp\":\"2026-04-29T12:00:00Z\",\"payload\":{\"id\":\"bound\"}}\n",
    )
    .expect("session should be written");
    let state = AppState {
        session_profile_bindings: BTreeMap::from([(
            "bound".to_string(),
            prodex_state::ResponseProfileBinding {
                profile_name: "main".to_string(),
                bound_at: 1,
            },
        )]),
        ..AppState::default()
    };

    let reports = collect_session_reports(&root, None, &state).expect("sessions collect");

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].profile.as_deref(), Some("main"));

    let _ = fs::remove_dir_all(root);
}

fn test_temp_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "prodex-session-store-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("old temp dir should be removed");
    }
    fs::create_dir_all(&root).expect("temp dir should be created");
    root
}
