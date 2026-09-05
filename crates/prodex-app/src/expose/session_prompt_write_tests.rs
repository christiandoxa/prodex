use super::session_prompt_write::{
    ExistingSessionPromptWrite, OpenProcessFile, ProcessDetails, ProcessInspector, ProcessRecord,
    ProcessState, QueueControl, QueueInvocation, ResolvedTarget, SessionPromptWriteError,
    SessionPromptWriteRequest, SessionPromptWriteService, exact_open_database, is_codex_writer,
    is_descendant_of, is_plain_prodex_session, legacy_thread_id, modern_thread_id,
    resolve_thread_identity,
};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "session_prompt_write_output_tests.rs"]
mod output_tests;

const THREAD: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216c4";

struct Fixture {
    root: PathBuf,
    workspace: PathBuf,
    _queue_db: PathBuf,
    _state_db: PathBuf,
    rollout: PathBuf,
    writer: ProcessDetails,
    records: Vec<ProcessRecord>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "prodex-session-prompt-write-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let workspace = root.join("workspace");
    let codex_home = root.join("target-codex-home");
    let queue_db = root.join("queue_1.sqlite");
    let state_db = root.join("state_5.sqlite");
    let rollout = codex_home
        .join("sessions/2026/09/03")
        .join(format!("rollout-{THREAD}.jsonl"));
    std::fs::create_dir_all(rollout.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(&queue_db, []).unwrap();
    std::fs::write(&state_db, []).unwrap();
    std::fs::write(
        &rollout,
        concat!(
            "{\"timestamp\":\"2026-09-03T10:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"agent_message\",\"message\":\"first output\"}}\n",
            "{\"timestamp\":\"2026-09-03T10:00:01Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"shell\",\"arguments\":\"{\\\"cmd\\\":\\\"true\\\"}\"}}\n",
            "{\"timestamp\":\"2026-09-03T10:00:02Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"output\":\"done\"}}\n"
        ),
    )
    .unwrap();
    let prodex = process(
        100,
        1,
        "/usr/bin/prodex",
        vec!["prodex", "s"],
        &workspace,
        10,
    );
    let writer = process(200, 100, "/usr/bin/codex", vec!["codex"], &workspace, 20);
    let environment = BTreeMap::from([
        ("HOME".to_string(), "/home/test-user".to_string()),
        ("CODEX_HOME".to_string(), codex_home.display().to_string()),
        ("CODEX_SQLITE_HOME".to_string(), root.display().to_string()),
        ("PWD".to_string(), workspace.display().to_string()),
        ("SECRET_TOKEN".to_string(), "must-not-copy".to_string()),
    ]);
    let writer_details = ProcessDetails {
        record: writer.clone(),
        environment,
        open_files: vec![
            OpenProcessFile {
                path: codex_home
                    .join("thread-writer-locks")
                    .join(format!("{THREAD}.lock")),
            },
            OpenProcessFile {
                path: queue_db.clone(),
            },
            OpenProcessFile {
                path: state_db.clone(),
            },
        ],
    };
    Fixture {
        root,
        workspace,
        _queue_db: queue_db,
        _state_db: state_db,
        rollout,
        writer: writer_details,
        records: vec![prodex, writer],
    }
}

fn process(
    pid: u32,
    parent_pid: u32,
    executable: &str,
    argv: Vec<&str>,
    cwd: &Path,
    start_time: u64,
) -> ProcessRecord {
    ProcessRecord {
        pid,
        parent_pid,
        uid: 1000,
        state: ProcessState::Running,
        executable: executable.into(),
        argv: argv.into_iter().map(str::to_string).collect(),
        cwd: cwd.to_path_buf(),
        start_time: Some(start_time),
        birth_identity: Some(format!("test:{start_time}")),
    }
}

struct FakeProcessInspector {
    uid: u32,
    records: Vec<ProcessRecord>,
    details: ProcessDetails,
    changed_records: Option<Vec<ProcessRecord>>,
    lists: AtomicUsize,
}

impl ProcessInspector for FakeProcessInspector {
    fn current_uid(&self) -> Result<u32, SessionPromptWriteError> {
        Ok(self.uid)
    }

    fn list(&self) -> Result<Vec<ProcessRecord>, SessionPromptWriteError> {
        let list = self.lists.fetch_add(1, Ordering::SeqCst);
        Ok(if list >= 2 {
            self.changed_records
                .clone()
                .unwrap_or_else(|| self.records.clone())
        } else {
            self.records.clone()
        })
    }

    fn inspect(&self, _pid: u32) -> Result<Option<ProcessDetails>, SessionPromptWriteError> {
        Ok(Some(self.details.clone()))
    }
}

struct FakeQueueControl {
    capability: bool,
    persisted: std::sync::atomic::AtomicBool,
    loaded_addressable: std::sync::atomic::AtomicBool,
    addressable_after: Option<usize>,
    addressability_checks: AtomicUsize,
    rollout: Option<PathBuf>,
    invocation: QueueInvocation,
    consumed_message: Option<String>,
    calls: Arc<Mutex<Vec<(String, String)>>>,
}

impl QueueControl for FakeQueueControl {
    fn check_capability(&self, _target: &ResolvedTarget) -> Result<(), SessionPromptWriteError> {
        self.capability
            .then_some(())
            .ok_or(SessionPromptWriteError::QueueUnsupported)
    }

    fn persisted_thread(
        &self,
        _state_db: &Path,
        _thread_id: &str,
    ) -> Result<bool, SessionPromptWriteError> {
        Ok(self.persisted.load(Ordering::SeqCst))
    }

    fn loaded_thread_addressable(
        &self,
        _target: &ResolvedTarget,
    ) -> Result<bool, SessionPromptWriteError> {
        let check = self.addressability_checks.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(self.loaded_addressable.load(Ordering::SeqCst)
            || self.addressable_after.is_some_and(|after| check >= after))
    }

    fn rollout_path(
        &self,
        _state_db: &Path,
        _thread_id: &str,
    ) -> Result<Option<PathBuf>, SessionPromptWriteError> {
        Ok(self.rollout.clone())
    }

    fn queue_once(&self, target: &ResolvedTarget, message: &str) -> QueueInvocation {
        self.calls
            .lock()
            .unwrap()
            .push((target.thread_id.clone(), message.to_string()));
        self.persisted.store(true, Ordering::SeqCst);
        if let (Some(path), Some(consumed_message)) = (&self.rollout, &self.consumed_message) {
            let line = serde_json::json!({
                "timestamp": "2026-09-03T10:00:03Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "role": "user",
                    "content": [{"type": "input_text", "text": consumed_message}]
                }
            });
            writeln!(
                std::fs::OpenOptions::new().append(true).open(path).unwrap(),
                "{line}"
            )
            .unwrap();
        }
        self.invocation.clone()
    }
}

fn service(
    fixture: &Fixture,
    queue: FakeQueueControl,
) -> SessionPromptWriteService<FakeProcessInspector, FakeQueueControl> {
    SessionPromptWriteService::with_adapters(
        FakeProcessInspector {
            uid: 1000,
            records: fixture.records.clone(),
            details: fixture.writer.clone(),
            changed_records: None,
            lists: AtomicUsize::new(0),
        },
        queue,
    )
}

fn request(fixture: &Fixture, message: &str) -> SessionPromptWriteRequest {
    SessionPromptWriteRequest {
        workspace_root: fixture.workspace.clone(),
        message: message.to_string(),
        cwd: None,
        prodex_pid: None,
        thread_id: None,
        binding_key: "test".to_string(),
    }
}

fn queue(fixture: &Fixture, message_id: Option<&str>) -> FakeQueueControl {
    FakeQueueControl {
        capability: true,
        persisted: std::sync::atomic::AtomicBool::new(true),
        loaded_addressable: std::sync::atomic::AtomicBool::new(false),
        addressable_after: None,
        addressability_checks: AtomicUsize::new(0),
        rollout: Some(fixture.rollout.clone()),
        invocation: QueueInvocation {
            succeeded: true,
            exit_code: Some(0),
            message_id: message_id.map(str::to_string),
            queued: false,
        },
        consumed_message: None,
        calls: Arc::new(Mutex::new(Vec::new())),
    }
}

#[test]
fn modern_lock_is_authoritative_and_legacy_rollout_is_supported() {
    let modern = PathBuf::from(format!("/tmp/thread-writer-locks/{THREAD}.lock"));
    let legacy = PathBuf::from(format!("/tmp/rollout-2026-09-03T10-00-00-{THREAD}.jsonl"));
    assert_eq!(modern_thread_id(&modern).as_deref(), Some(THREAD));
    assert_eq!(legacy_thread_id(&legacy).as_deref(), Some(THREAD));
    assert_eq!(
        resolve_thread_identity(&[
            OpenProcessFile { path: modern },
            OpenProcessFile { path: legacy },
        ]),
        Ok(THREAD.to_string())
    );
}

#[test]
fn mismatched_lock_and_rollout_fail_closed_without_inference() {
    let other = "019f3b59-7771-7ea1-a9a1-3cd638f216c5";
    let error = resolve_thread_identity(&[
        OpenProcessFile {
            path: PathBuf::from(format!("/tmp/thread-writer-locks/{THREAD}.lock")),
        },
        OpenProcessFile {
            path: PathBuf::from(format!("/tmp/rollout-{other}.jsonl")),
        },
    ])
    .unwrap_err();
    assert_eq!(error, SessionPromptWriteError::ThreadIdentityConflict);
    assert_eq!(
        resolve_thread_identity(&[]),
        Err(SessionPromptWriteError::ThreadIdentityUnavailable)
    );
}

#[test]
fn process_matching_requires_user_cwd_and_plain_s() {
    let cwd = PathBuf::from("/home/test-user/project");
    let plain = process(1, 0, "/usr/bin/prodex", vec!["prodex", "s"], &cwd, 1);
    assert!(is_plain_prodex_session(&plain));
    for argv in [
        vec!["prodex", "s", "expose"],
        vec!["prodex", "super"],
        vec!["prodex", "s", "exec"],
        vec!["prodex", "s", "prodex_super_start"],
        vec!["prodex", "s", "--release-smoke"],
    ] {
        assert!(!is_plain_prodex_session(&process(
            1,
            0,
            "/usr/bin/prodex",
            argv,
            &cwd,
            1
        )));
    }
    assert!(is_plain_prodex_session(&process(
        1,
        0,
        "/usr/bin/prodex",
        vec!["prodex", "s"],
        Path::new("/home/test-user/other"),
        1,
    )));
}

#[test]
fn session_prompt_write_accepts_equivalent_cwd_path_spellings() {
    let mut fixture = fixture();
    fixture.records[0].cwd = fixture.workspace.join("..").join("workspace");
    let message_id = "019f3b59-7771-7ea1-a9a1-3cd638f216c5";
    let mut queue_control = queue(&fixture, Some(message_id));
    queue_control.consumed_message = Some("hello".to_string());

    let result = service(&fixture, queue_control)
        .write(request(&fixture, "hello"))
        .expect("equivalent cwd should resolve to the session");

    assert_eq!(result.thread_id, THREAD);
}

#[test]
fn session_prompt_write_rejects_mismatched_cwd_without_no_session_fallback() {
    let fixture = fixture();
    let mut prompt = request(&fixture, "wrong workspace");
    prompt.cwd = Some(fixture.root.display().to_string());

    assert_eq!(
        service(&fixture, queue(&fixture, None))
            .write(prompt)
            .unwrap_err(),
        SessionPromptWriteError::StaleTarget
    );
}

#[test]
fn stale_rollout_path_is_rejected_before_prompt_write() {
    let fixture = fixture();
    let stale = fixture
        .root
        .parent()
        .unwrap()
        .join(format!("rollout-stale-{THREAD}.jsonl"));
    std::fs::write(&stale, b"stale\n").unwrap();
    let mut queue_control = queue(&fixture, None);
    queue_control.rollout = Some(stale.clone());
    let calls = Arc::clone(&queue_control.calls);

    assert_eq!(
        service(&fixture, queue_control)
            .write(request(&fixture, "must not write"))
            .unwrap_err(),
        SessionPromptWriteError::OutputSourceChanged
    );
    assert!(calls.lock().unwrap().is_empty());
    let _ = std::fs::remove_file(stale);
}

#[test]
fn writer_requires_codex_role_and_prodex_ancestry() {
    let cwd = PathBuf::from("/home/test-user/project");
    let prodex = process(10, 1, "/usr/bin/prodex", vec!["prodex", "s"], &cwd, 1);
    let wrapper = process(20, 10, "/usr/bin/node", vec!["node", "codex"], &cwd, 2);
    let writer = process(30, 20, "/usr/bin/codex", vec!["codex"], &cwd, 3);
    let processes = [prodex.clone(), wrapper, writer.clone()];
    let by_pid = processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    assert!(is_codex_writer(&writer));
    assert!(is_descendant_of(writer.pid, prodex.pid, &by_pid));
    let remote = process(
        31,
        20,
        "/usr/bin/codex",
        vec!["codex", "--remote", "unix:///tmp/x"],
        &cwd,
        4,
    );
    assert!(!is_codex_writer(&remote));
}

#[test]
fn ambiguous_session_and_writer_never_call_queue() {
    let mut session_fixture = fixture();
    let second = process(
        101,
        1,
        "/usr/bin/prodex",
        vec!["prodex", "s"],
        &session_fixture.workspace,
        11,
    );
    session_fixture.records.insert(1, second);
    let queue_control = queue(
        &session_fixture,
        Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"),
    );
    let error = service(&session_fixture, queue_control).write(request(&session_fixture, "hello"));
    assert_eq!(
        error.unwrap_err(),
        SessionPromptWriteError::AmbiguousSession
    );

    let mut writer_fixture = fixture();
    let second = process(
        201,
        100,
        "/usr/bin/codex",
        vec!["codex"],
        &writer_fixture.workspace,
        21,
    );
    writer_fixture.records.push(second);
    let queue_control = queue(
        &writer_fixture,
        Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"),
    );
    let error = service(&writer_fixture, queue_control).write(request(&writer_fixture, "hello"));
    assert_eq!(
        error.unwrap_err(),
        SessionPromptWriteError::AmbiguousCodexWriter
    );
}

#[test]
fn queue_success_preserves_multiline_message_and_same_thread() {
    let fixture = fixture();
    let message_id = "019f3b59-7771-7ea1-a9a1-3cd638f216c5";
    let mut queue_control = queue(&fixture, Some(message_id));
    queue_control.consumed_message = Some("line one\nline two".to_string());
    let calls = Arc::clone(&queue_control.calls);
    let result = service(&fixture, queue_control).write(request(&fixture, "line one\nline two"));
    assert_eq!(result.unwrap().thread_id, THREAD);
    assert_eq!(
        calls.lock().unwrap()[0],
        (THREAD.to_string(), "line one\nline two".to_string())
    );
}

#[test]
fn fresh_idle_thread_uses_live_app_server_before_first_persisted_row() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"));
    queue_control.persisted.store(false, Ordering::SeqCst);
    queue_control
        .loaded_addressable
        .store(true, Ordering::SeqCst);
    queue_control.consumed_message = Some("first prompt before manual input".to_string());

    let calls = Arc::clone(&queue_control.calls);
    let result = service(&fixture, queue_control)
        .write(request(&fixture, "first prompt before manual input"))
        .expect("live app-server thread should be queueable before persisted rollout");

    assert_eq!(result.thread_id, THREAD);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn addressability_race_waits_without_becoming_no_session() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"));
    queue_control.persisted.store(false, Ordering::SeqCst);
    queue_control.addressable_after = Some(2);
    queue_control.consumed_message = Some("hello".to_string());

    let result = service(&fixture, queue_control).write(request(&fixture, "hello"));

    assert_eq!(result.unwrap().thread_id, THREAD);
}

#[test]
fn queue_success_requires_the_same_rollout_user_event() {
    let fixture = fixture();
    let message_id = "019f3b59-7771-7ea1-a9a1-3cd638f216c5";
    let mut queue_control = queue(&fixture, Some(message_id));
    queue_control.consumed_message = Some("already consumed".to_string());
    let result = service(&fixture, queue_control)
        .write(request(&fixture, "already consumed"))
        .unwrap();
    assert_eq!(result.verification, "rollout_user_event_observed");
}

#[test]
fn app_server_turn_control_requires_the_rollout_user_event_before_success() {
    let fixture = fixture();
    let message = "visible user message";
    let mut queue_control = queue(&fixture, None);
    queue_control.persisted.store(false, Ordering::SeqCst);
    queue_control
        .loaded_addressable
        .store(true, Ordering::SeqCst);
    queue_control.consumed_message = Some(message.to_string());

    let result = service(&fixture, queue_control)
        .write(request(&fixture, message))
        .expect("turn control must wait for the same rollout user event");

    assert_eq!(result.verification, "rollout_user_event_observed");
}

#[test]
fn unpersisted_thread_reproduces_codex_0153_regression_without_queue_mutation() {
    let fixture = fixture();
    let queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"));
    queue_control.persisted.store(false, Ordering::SeqCst);
    let calls = Arc::clone(&queue_control.calls);
    let error = service(&fixture, queue_control).write(request(&fixture, "must not send"));
    assert_eq!(
        error.unwrap_err(),
        SessionPromptWriteError::SessionNotQueueAddressable
    );
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn missing_main_queue_db_and_wal_only_are_rejected() {
    let root = std::env::temp_dir().join(format!("prodex-db-check-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let files = vec![OpenProcessFile {
        path: root.join("queue_1.sqlite-wal"),
    }];
    assert_eq!(
        exact_open_database(&files, super::session_prompt_write::DatabaseKind::Queue).unwrap(),
        None
    );
    let _ = std::fs::remove_dir_all(root);
}
