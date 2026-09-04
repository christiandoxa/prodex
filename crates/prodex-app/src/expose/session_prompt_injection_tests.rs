use super::session_prompt_injection::{
    ExistingSessionPromptInjector, OpenProcessFile, ProcessDetails, ProcessInspector,
    ProcessRecord, ProcessState, PromptInjectionError, PromptInjectionRequest,
    PromptOutputReadRequest, QueueControl, QueueInvocation, QueueSnapshot, ResolvedTarget,
    SessionPromptInjectionService, SystemQueueControl, exact_open_database, is_codex_writer,
    is_descendant_of, is_plain_prodex_session, legacy_thread_id, modern_thread_id,
    read_output_events, resolve_thread_identity, valid_rollout_path_in_authoritative_open_files,
    valid_rollout_path_in_roots,
};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

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
        "prodex-session-injection-test-{}-{}",
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
    fn current_uid(&self) -> Result<u32, PromptInjectionError> {
        Ok(self.uid)
    }

    fn list(&self) -> Result<Vec<ProcessRecord>, PromptInjectionError> {
        let list = self.lists.fetch_add(1, Ordering::SeqCst);
        Ok(if list >= 2 {
            self.changed_records
                .clone()
                .unwrap_or_else(|| self.records.clone())
        } else {
            self.records.clone()
        })
    }

    fn inspect(&self, _pid: u32) -> Result<Option<ProcessDetails>, PromptInjectionError> {
        Ok(Some(self.details.clone()))
    }
}

struct FakeQueueControl {
    capability: bool,
    persisted: bool,
    rollout: Option<PathBuf>,
    snapshots: Mutex<VecDeque<QueueSnapshot>>,
    invocation: QueueInvocation,
    consumed_message: Option<String>,
    calls: Arc<Mutex<Vec<(String, String)>>>,
}

impl QueueControl for FakeQueueControl {
    fn check_capability(&self, _target: &ResolvedTarget) -> Result<(), PromptInjectionError> {
        self.capability
            .then_some(())
            .ok_or(PromptInjectionError::QueueUnsupported)
    }

    fn persisted_thread(
        &self,
        _state_db: &Path,
        _thread_id: &str,
    ) -> Result<bool, PromptInjectionError> {
        Ok(self.persisted)
    }

    fn rollout_path(
        &self,
        _state_db: &Path,
        _thread_id: &str,
    ) -> Result<Option<PathBuf>, PromptInjectionError> {
        Ok(self.rollout.clone())
    }

    fn snapshot(
        &self,
        _queue_db: &Path,
        _thread_id: &str,
    ) -> Result<QueueSnapshot, PromptInjectionError> {
        Ok(self
            .snapshots
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default())
    }

    fn queue_once(&self, target: &ResolvedTarget, message: &str) -> QueueInvocation {
        self.calls
            .lock()
            .unwrap()
            .push((target.thread_id.clone(), message.to_string()));
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
) -> SessionPromptInjectionService<FakeProcessInspector, FakeQueueControl> {
    SessionPromptInjectionService::with_adapters(
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

fn request(fixture: &Fixture, message: &str) -> PromptInjectionRequest {
    PromptInjectionRequest {
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
        persisted: true,
        rollout: Some(fixture.rollout.clone()),
        snapshots: Mutex::new(VecDeque::from([
            QueueSnapshot::default(),
            QueueSnapshot {
                item_ids: message_id.into_iter().map(str::to_string).collect(),
            },
        ])),
        invocation: QueueInvocation {
            succeeded: true,
            exit_code: Some(0),
            message_id: message_id.map(str::to_string),
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
    assert_eq!(error, PromptInjectionError::ThreadIdentityConflict);
    assert_eq!(
        resolve_thread_identity(&[]),
        Err(PromptInjectionError::ThreadIdentityUnavailable)
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
    let error = service(&session_fixture, queue_control).inject(request(&session_fixture, "hello"));
    assert_eq!(error.unwrap_err(), PromptInjectionError::AmbiguousSession);

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
    let error = service(&writer_fixture, queue_control).inject(request(&writer_fixture, "hello"));
    assert_eq!(
        error.unwrap_err(),
        PromptInjectionError::AmbiguousCodexWriter
    );
}

#[test]
fn queue_success_preserves_multiline_message_and_same_thread() {
    let fixture = fixture();
    let message_id = "019f3b59-7771-7ea1-a9a1-3cd638f216c5";
    let queue_control = queue(&fixture, Some(message_id));
    let calls = Arc::clone(&queue_control.calls);
    let result = service(&fixture, queue_control).inject(request(&fixture, "line one\nline two"));
    assert_eq!(result.unwrap().thread_id, THREAD);
    assert_eq!(
        calls.lock().unwrap()[0],
        (THREAD.to_string(), "line one\nline two".to_string())
    );
}

#[test]
fn queue_acknowledgement_is_valid_when_item_is_consumed_before_snapshot() {
    let fixture = fixture();
    let message_id = "019f3b59-7771-7ea1-a9a1-3cd638f216c5";
    let mut queue_control = queue(&fixture, Some(message_id));
    queue_control.consumed_message = Some("already consumed".to_string());
    queue_control.snapshots = Mutex::new(VecDeque::from([
        QueueSnapshot::default(),
        QueueSnapshot::default(),
    ]));
    let result = service(&fixture, queue_control)
        .inject(request(&fixture, "already consumed"))
        .unwrap();
    assert_eq!(result.verification, "consumed_rollout");
}

#[test]
fn system_queue_snapshot_reads_current_queue_schema() {
    let root = std::env::temp_dir().join(format!(
        "prodex-session-queue-snapshot-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let queue_db = root.join("queue_1.sqlite");
    let connection = rusqlite::Connection::open(&queue_db).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE queued_items (
                id TEXT NOT NULL,
                thread_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                queue_order INTEGER NOT NULL,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL
            );
            ",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO queued_items
                (id, thread_id, payload_json, queue_order, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, '{}', 1, 1, 1)",
            rusqlite::params!["message-id", THREAD],
        )
        .unwrap();
    drop(connection);

    let snapshot = SystemQueueControl.snapshot(&queue_db, THREAD).unwrap();
    assert!(snapshot.item_ids.contains("message-id"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unpersisted_thread_reproduces_codex_0153_regression_without_queue_mutation() {
    let fixture = fixture();
    let mut queue_control = queue(&fixture, Some("019f3b59-7771-7ea1-a9a1-3cd638f216c5"));
    queue_control.persisted = false;
    let calls = Arc::clone(&queue_control.calls);
    let error = service(&fixture, queue_control).inject(request(&fixture, "must not send"));
    assert_eq!(
        error.unwrap_err(),
        PromptInjectionError::SessionNotQueueAddressable
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
        exact_open_database(&files, super::session_prompt_injection::DatabaseKind::Queue).unwrap(),
        None
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn output_read_uses_exact_rollout_cursor_and_repeated_cursor_is_safe() {
    let fixture = fixture();
    let service = service(&fixture, queue(&fixture, None));
    let first = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "test".to_string(),
        })
        .unwrap();
    assert_eq!(first.thread_id, THREAD);
    assert_eq!(first.events[0].kind, "assistant");
    assert_eq!(first.events[1].kind, "tool");
    let repeated = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor.clone()),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "test".to_string(),
        })
        .unwrap();
    let repeated_again = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some(first.next_cursor),
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "test".to_string(),
        })
        .unwrap();
    assert_eq!(repeated, repeated_again);
}

#[test]
fn output_cursor_rejects_changed_process_identity_and_invalid_values() {
    let fixture = fixture();
    let service = service(&fixture, queue(&fixture, None));
    let first = service
        .read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: None,
            limit: 10,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "test".to_string(),
        })
        .unwrap();
    assert_eq!(
        service.read_output(PromptOutputReadRequest {
            workspace_root: fixture.workspace.clone(),
            cursor: Some("not-a-cursor".to_string()),
            limit: 1,
            wait_ms: 0,
            prodex_pid: None,
            thread_id: None,
            binding_key: "test".to_string(),
        }),
        Err(PromptInjectionError::InvalidCursor)
    );
    assert!(!first.next_cursor.is_empty());
}

#[test]
fn implementation_has_no_direct_queue_payload_or_pty_injection_path() {
    let source = include_str!("session_prompt_injection.rs");
    for forbidden in [
        "payload_json",
        "INSERT INTO queued_items",
        "xdotool",
        "TIOCSTI",
        "/dev/pts/",
        "prodex_super_start",
        "/expose/input",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden mechanism: {forbidden}"
        );
    }
}

#[test]
fn output_page_does_not_drop_events_from_one_rollout_line() {
    let fixture = fixture();
    let first = read_output_events(&fixture.rollout, 0, 0, 1).unwrap();
    assert_eq!(first.events.len(), 1);
    assert!(first.next_offset > 0);
    assert_eq!(first.next_event_index, 0);
    let second = read_output_events(
        &fixture.rollout,
        first.next_offset,
        first.next_event_index,
        10,
    )
    .unwrap();
    assert_eq!(second.events.len(), 2);
    assert!(second.next_offset > 0);
}

#[test]
fn output_source_accepts_managed_session_directory_symlink() {
    let fixture = fixture();
    let overlay = fixture.root.join("overlay");
    std::fs::create_dir_all(&overlay).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        fixture.root.join("target-codex-home/sessions"),
        overlay.join("sessions"),
    )
    .unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(
        fixture.root.join("target-codex-home/sessions"),
        overlay.join("sessions"),
    )
    .unwrap();

    let stored = overlay.join("sessions/2026/09/03").join(
        fixture
            .rollout
            .file_name()
            .expect("fixture rollout should have a filename"),
    );
    assert_eq!(
        valid_rollout_path_in_roots(&stored, std::slice::from_ref(&overlay), THREAD),
        None
    );
    assert_eq!(
        valid_rollout_path_in_authoritative_open_files(
            &stored,
            std::slice::from_ref(&overlay),
            &[OpenProcessFile {
                path: fixture.rollout.clone(),
            }],
            THREAD,
        ),
        Some(fixture.rollout.canonicalize().unwrap())
    );
    assert_eq!(
        valid_rollout_path_in_authoritative_open_files(&stored, &[overlay], &[], THREAD),
        None
    );
}

#[test]
fn output_source_rejects_symlink_and_hidden_instruction_rollout() {
    let fixture = fixture();
    let outside = fixture.root.join("outside.jsonl");
    std::fs::write(&outside, "secret\n").unwrap();
    let link = fixture
        .rollout
        .parent()
        .unwrap()
        .join(format!("rollout-{THREAD}-link.jsonl"));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert!(
            valid_rollout_path_in_roots(&link, &[fixture.root.join("target-codex-home")], THREAD,)
                .is_none()
        );
    }
    let hidden = fixture.root.join("hidden.jsonl");
    std::fs::write(
        &hidden,
        "{\"timestamp\":\"2026-09-03T10:00:00Z\",\"type\":\"session_meta\",\"payload\":{\"base_instructions\":{\"text\":\"do not disclose\"}}}\n",
    )
    .unwrap();
    let batch = read_output_events(&hidden, 0, 0, 10).unwrap();
    assert!(batch.events.is_empty());
}
