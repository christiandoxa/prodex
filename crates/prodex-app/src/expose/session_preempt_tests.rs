use super::session_prompt_write::{
    ExistingSessionPromptWrite, ProcessRecord, ProcessState, PromptOutputReadRequest,
    PromptOutputReadSuccess, QueueControl, QueueInvocation, QueuePreemptResult, ResolvedTarget,
    SessionPreemptRequest, SessionPreemptSuccess, SessionPromptWriteError,
    SessionPromptWriteRequest, SessionPromptWriteService, SessionPromptWriteSuccess,
    SystemQueueControl, TargetEnvironment,
};
use super::session_prompt_write_e2e_tests::{call_tool, endpoint_at};
use super::session_prompt_write_tests::{fixture, process_inspector};
use crate::app_server_control::{AppServerRequestOutcome, connect_unix_socket, request_result};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};
use tungstenite::Message;

const THREAD_A: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216c4";
const THREAD_B: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216d4";
const TURN_A: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216e4";
const TURN_B: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216f4";
struct ServiceQueueControl {
    rollout: PathBuf,
    state: Arc<Mutex<ServiceQueueState>>,
    queue_entered: Option<Arc<Barrier>>,
    queue_release: Option<Arc<Barrier>>,
    block_queue: AtomicBool,
}
struct ServiceQueueState {
    active_turn: Option<String>,
    queue: Vec<(String, String)>,
    next_id: usize,
    enqueue_after_boundary: Option<String>,
}
impl QueueControl for ServiceQueueControl {
    fn check_capability(&self, _target: &ResolvedTarget) -> Result<(), SessionPromptWriteError> {
        Ok(())
    }

    fn persisted_thread(
        &self,
        _state_db: &Path,
        _thread_id: &str,
    ) -> Result<bool, SessionPromptWriteError> {
        Ok(true)
    }

    fn rollout_path(
        &self,
        _state_db: &Path,
        _thread_id: &str,
    ) -> Result<Option<PathBuf>, SessionPromptWriteError> {
        Ok(Some(self.rollout.clone()))
    }

    fn queue_once(&self, _target: &ResolvedTarget, message: &str) -> QueueInvocation {
        let id = {
            let mut state = self.state.lock().unwrap();
            state.next_id += 1;
            let id = format!("submission-{}", state.next_id);
            state.queue.push((id.clone(), message.to_string()));
            id
        };
        if self.block_queue.swap(false, Ordering::SeqCst) {
            if let Some(barrier) = &self.queue_entered {
                barrier.wait();
            }
            if let Some(barrier) = &self.queue_release {
                barrier.wait();
            }
        }
        QueueInvocation::accepted(Some(0), Some(id.clone()), Some(id), true)
    }

    fn preempt(
        &self,
        _target: &ResolvedTarget,
    ) -> Result<QueuePreemptResult, SessionPromptWriteError> {
        let mut state = self.state.lock().unwrap();
        let current_turn_id = state.active_turn.take();
        let cancelled_submission_ids = state.queue.drain(..).map(|(id, _)| id).collect::<Vec<_>>();
        let queue_empty_at_boundary = true;
        if let Some(message) = state.enqueue_after_boundary.take() {
            state.next_id += 1;
            let id = format!("submission-{}", state.next_id);
            state.queue.push((id, message));
        }
        Ok(QueuePreemptResult {
            current_turn_id: current_turn_id.clone(),
            current_turn_interrupted: current_turn_id.is_some(),
            cancelled_submission_ids,
            remaining_submission_ids: state.queue.iter().map(|(id, _)| id.clone()).collect(),
            queue_empty_at_boundary,
            session_ready: state.active_turn.is_none() && state.queue.is_empty(),
        })
    }
}
fn preempt_request(fixture: &super::session_prompt_write_tests::Fixture) -> SessionPreemptRequest {
    SessionPreemptRequest {
        workspace_root: fixture.workspace.clone(),
        cwd: None,
        prodex_pid: None,
        thread_id: None,
        binding_key: "preempt-test".to_string(),
    }
}

fn service_queue(
    fixture: &super::session_prompt_write_tests::Fixture,
    active_turn: Option<&str>,
    queue: &[(&str, &str)],
) -> (ServiceQueueControl, Arc<Mutex<ServiceQueueState>>) {
    let state = Arc::new(Mutex::new(ServiceQueueState {
        active_turn: active_turn.map(str::to_string),
        queue: queue
            .iter()
            .map(|(id, message)| ((*id).to_string(), (*message).to_string()))
            .collect(),
        next_id: queue.len(),
        enqueue_after_boundary: None,
    }));
    (
        ServiceQueueControl {
            rollout: fixture.rollout.clone(),
            state: Arc::clone(&state),
            queue_entered: None,
            queue_release: None,
            block_queue: AtomicBool::new(false),
        },
        state,
    )
}

#[test]
fn preempt_interrupts_current_turn_and_cancels_all_pending_prompts() {
    let fixture = fixture();
    let (queue, state) = service_queue(
        &fixture,
        Some(TURN_A),
        &[("queued-a", "first"), ("queued-b", "second")],
    );
    let service = SessionPromptWriteService::with_adapters(process_inspector(&fixture), queue);

    let result = service.preempt(preempt_request(&fixture)).unwrap();

    assert_eq!(result.current_turn_id.as_deref(), Some(TURN_A));
    assert!(result.current_turn_interrupted);
    assert_eq!(result.cancelled_submission_ids, ["queued-a", "queued-b"]);
    assert!(result.remaining_submission_ids.is_empty());
    assert!(result.queue_empty_at_boundary);
    assert!(result.session_ready);
    assert!(state.lock().unwrap().queue.is_empty());
}

#[test]
fn preempt_without_current_turn_cancels_queue_and_repeats_safely() {
    let fixture = fixture();
    let (queue, state) = service_queue(
        &fixture,
        None,
        &[("queued-a", "first"), ("queued-b", "second")],
    );
    let service = SessionPromptWriteService::with_adapters(process_inspector(&fixture), queue);

    let first = service.preempt(preempt_request(&fixture)).unwrap();
    let second = service.preempt(preempt_request(&fixture)).unwrap();

    assert!(first.current_turn_id.is_none());
    assert!(!first.current_turn_interrupted);
    assert_eq!(first.cancelled_submission_ids.len(), 2);
    assert!(first.session_ready);
    assert!(second.current_turn_id.is_none());
    assert!(!second.current_turn_interrupted);
    assert!(second.cancelled_submission_ids.is_empty());
    assert!(second.session_ready);
    assert_eq!(second.generation, 2);
    assert!(state.lock().unwrap().queue.is_empty());
}

#[test]
fn enqueue_before_preempt_is_cancelled_and_enqueue_after_boundary_survives() {
    let fixture = fixture();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let state = Arc::new(Mutex::new(ServiceQueueState {
        active_turn: Some(TURN_A.to_string()),
        queue: Vec::new(),
        next_id: 0,
        enqueue_after_boundary: None,
    }));
    let queue = ServiceQueueControl {
        rollout: fixture.rollout.clone(),
        state: Arc::clone(&state),
        queue_entered: Some(Arc::clone(&entered)),
        queue_release: Some(Arc::clone(&release)),
        block_queue: AtomicBool::new(true),
    };
    let service = Arc::new(SessionPromptWriteService::with_adapters(
        process_inspector(&fixture),
        queue,
    ));
    let write_service = Arc::clone(&service);
    let write_fixture = fixture.workspace.clone();
    let writer = thread::spawn(move || {
        write_service.write(SessionPromptWriteRequest {
            workspace_root: write_fixture,
            message: "queued-before-preempt".to_string(),
            cwd: None,
            prodex_pid: None,
            thread_id: None,
            binding_key: "write-before".to_string(),
        })
    });
    entered.wait();
    let preempt_service = Arc::clone(&service);
    let preempt_fixture = fixture.workspace.clone();
    let preemptor = thread::spawn(move || {
        preempt_service.preempt(SessionPreemptRequest {
            workspace_root: preempt_fixture,
            cwd: None,
            prodex_pid: None,
            thread_id: None,
            binding_key: "preempt-after-write".to_string(),
        })
    });
    release.wait();

    let written = writer.join().unwrap().unwrap();
    let preempted = preemptor.join().unwrap().unwrap();
    assert_eq!(written.submission_id.as_deref(), Some("submission-1"));
    assert_eq!(preempted.cancelled_submission_ids, ["submission-1"]);
    assert!(preempted.current_turn_interrupted);
    assert!(state.lock().unwrap().queue.is_empty());

    let after = service
        .write(SessionPromptWriteRequest {
            workspace_root: fixture.workspace.clone(),
            message: "queued-after-boundary".to_string(),
            cwd: None,
            prodex_pid: None,
            thread_id: None,
            binding_key: "write-after".to_string(),
        })
        .unwrap();
    assert_eq!(after.submission_id.as_deref(), Some("submission-2"));
    assert_eq!(
        state.lock().unwrap().queue,
        [(
            "submission-2".to_string(),
            "queued-after-boundary".to_string()
        )]
    );
}

#[derive(Default)]
struct PublicPreemptBridge {
    requests: Mutex<Vec<SessionPreemptRequest>>,
}

impl ExistingSessionPromptWrite for PublicPreemptBridge {
    fn write(
        &self,
        _request: SessionPromptWriteRequest,
    ) -> Result<SessionPromptWriteSuccess, SessionPromptWriteError> {
        Err(SessionPromptWriteError::NoSession)
    }

    fn read_output(
        &self,
        _request: PromptOutputReadRequest,
    ) -> Result<PromptOutputReadSuccess, SessionPromptWriteError> {
        Err(SessionPromptWriteError::NoSession)
    }

    fn preempt(
        &self,
        request: SessionPreemptRequest,
    ) -> Result<SessionPreemptSuccess, SessionPromptWriteError> {
        self.requests.lock().unwrap().push(request);
        Ok(SessionPreemptSuccess {
            prodex_pid: 100,
            codex_pid: 200,
            thread_id: THREAD_A.to_string(),
            current_turn_id: Some(TURN_A.to_string()),
            current_turn_interrupted: true,
            cancelled_submission_ids: vec!["queued-a".to_string(), "queued-b".to_string()],
            remaining_submission_ids: Vec::new(),
            queue_empty_at_boundary: true,
            session_ready: true,
            generation: 1,
        })
    }
}

#[test]
fn preempt_is_exposed_through_the_public_mcp_control_plane() {
    let bridge = Arc::new(PublicPreemptBridge::default());
    let requests = Arc::clone(&bridge);
    let capability = "preempt-capability-000000000000000000000";
    let (address, shared, mut server) = endpoint_at(
        "pdxi_preempt",
        capability,
        std::env::current_dir().unwrap(),
        bridge,
    );
    let value = call_tool(
        address,
        &format!("/pdx/v1/{capability}/mcp"),
        1,
        "prodex_session_preempt",
        json!({"thread_id": THREAD_A}),
        "preempt-session",
    );
    assert_eq!(value["status"], "preempted");
    assert_eq!(value["current_turn_interrupted"], true);
    assert_eq!(value["cancelled_count"], 2);
    assert_eq!(value["queue_empty_at_boundary"], true);
    assert_eq!(value["session_ready"], true);
    assert_eq!(
        requests.requests.lock().unwrap()[0].thread_id.as_deref(),
        Some(THREAD_A)
    );
    server.shutdown();
    shared.pty.shutdown();
    shared.mcp.as_ref().unwrap().run_manager.shutdown();
}

#[derive(Clone)]
struct LiveThread {
    active_turn: Option<String>,
    queue: Vec<String>,
}

#[derive(Default)]
struct LiveServerState {
    threads: Mutex<BTreeMap<String, LiveThread>>,
}

struct FakeAppServer {
    root: PathBuf,
    socket_path: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    state: Arc<LiveServerState>,
    codex_home: PathBuf,
    workspace: PathBuf,
}

impl FakeAppServer {
    fn new(threads: impl IntoIterator<Item = (&'static str, LiveThread)>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "prodex-session-preempt-live-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let codex_home = root.join("codex-home");
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&codex_home).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(root.join("queue_1.sqlite"), []).unwrap();
        std::fs::write(root.join("state_5.sqlite"), []).unwrap();
        let socket_path = root.join(".s");
        let listener = UnixListener::bind(&socket_path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let state = Arc::new(LiveServerState {
            threads: Mutex::new(
                threads
                    .into_iter()
                    .map(|(id, thread)| (id.to_string(), thread))
                    .collect(),
            ),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker_state = Arc::clone(&state);
        let worker_home = codex_home.clone();
        let worker_workspace = workspace.clone();
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => serve_app_server_connection(
                        stream,
                        &worker_state,
                        &worker_home,
                        &worker_workspace,
                    ),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(1));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            root,
            socket_path,
            stop,
            worker: Some(worker),
            state,
            codex_home,
            workspace,
        }
    }

    fn target(&self, thread_id: &str) -> ResolvedTarget {
        let endpoint = format!("unix://{}", self.socket_path.display());
        ResolvedTarget {
            prodex: live_process(
                100,
                1,
                "/usr/bin/prodex",
                vec!["prodex", "s"],
                &self.workspace,
                10,
            ),
            writer: live_process(
                200,
                100,
                "/usr/bin/codex",
                vec!["codex", "app-server", "--listen", &endpoint],
                &self.workspace,
                20,
            ),
            thread_id: thread_id.to_string(),
            queue_db: self.root.join("queue_1.sqlite"),
            state_db: self.root.join("state_5.sqlite"),
            environment: TargetEnvironment {
                home: "/home/test-user".to_string(),
                codex_home: self.codex_home.clone(),
                codex_sqlite_home: self.root.clone(),
                pwd: self.workspace.display().to_string(),
            },
            remote_endpoint: Some(endpoint),
        }
    }
}

impl Drop for FakeAppServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn live_process(
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

fn serve_app_server_connection(
    stream: std::os::unix::net::UnixStream,
    state: &LiveServerState,
    codex_home: &Path,
    workspace: &Path,
) {
    let Ok(mut socket) = tungstenite::accept(stream) else {
        return;
    };
    while let Ok(message) = socket.read() {
        let Message::Text(text) = message else {
            continue;
        };
        let Ok(request) = serde_json::from_str::<Value>(text.as_ref()) else {
            return;
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let result = live_request_result(method, &request, state, codex_home, workspace);
        let response = match result {
            Ok(result) => json!({"id": id, "result": result}),
            Err(message) => json!({
                "id": id,
                "error": {"code": -32602, "message": message}
            }),
        };
        if socket
            .send(Message::Text(response.to_string().into()))
            .is_err()
        {
            return;
        }
    }
}

fn live_request_result(
    method: &str,
    request: &Value,
    state: &LiveServerState,
    codex_home: &Path,
    workspace: &Path,
) -> Result<Value, String> {
    match method {
        "initialize" => Ok(json!({
            "codexHome": codex_home,
            "platformFamily": "unix",
            "platformOs": "linux",
            "userAgent": "preempt-test"
        })),
        "thread/read" => {
            let thread_id = request["params"]["threadId"]
                .as_str()
                .ok_or_else(|| "missing thread id".to_string())?;
            let thread = state
                .threads
                .lock()
                .unwrap()
                .get(thread_id)
                .cloned()
                .ok_or_else(|| "thread not found".to_string())?;
            Ok(json!({
                "thread": {
                    "id": thread_id,
                    "sessionId": thread_id,
                    "ephemeral": false,
                    "canAcceptDirectInput": true,
                    "cwd": workspace,
                    "status": if thread.active_turn.is_some() { json!({"type": "active", "activeFlags": []}) } else { json!({"type": "idle"}) },
                    "turns": []
                }
            }))
        }
        "thread/turns/list" => {
            let thread_id = request["params"]["threadId"]
                .as_str()
                .ok_or_else(|| "missing thread id".to_string())?;
            let thread = state
                .threads
                .lock()
                .unwrap()
                .get(thread_id)
                .cloned()
                .ok_or_else(|| "thread not found".to_string())?;
            Ok(json!({
                "data": thread
                    .active_turn
                    .as_ref()
                    .map(|id| vec![json!({"id": id, "status": {"type": "inProgress"}})])
                    .unwrap_or_default(),
                "nextCursor": null,
            }))
        }
        "thread/queue/list" => {
            let thread_id = request["params"]["threadId"]
                .as_str()
                .ok_or_else(|| "missing thread id".to_string())?;
            let threads = state.threads.lock().unwrap();
            let thread = threads
                .get(thread_id)
                .ok_or_else(|| "thread not found".to_string())?;
            Ok(json!({
                "data": thread.queue.iter().map(|id| json!({
                    "id": id,
                    "input": [{"type": "text", "text": id, "textElements": []}],
                    "clientUserMessageId": id
                })).collect::<Vec<_>>(),
                "nextCursor": null
            }))
        }
        "thread/queue/delete" => {
            let thread_id = request["params"]["threadId"]
                .as_str()
                .ok_or_else(|| "missing thread id".to_string())?;
            let submission_id = request["params"]["queuedSubmissionId"]
                .as_str()
                .ok_or_else(|| "missing submission id".to_string())?;
            let mut threads = state.threads.lock().unwrap();
            let thread = threads
                .get_mut(thread_id)
                .ok_or_else(|| "thread not found".to_string())?;
            let before = thread.queue.len();
            thread.queue.retain(|id| id != submission_id);
            Ok(json!({"deleted": thread.queue.len() != before}))
        }
        "thread/queue/add" => {
            let thread_id = request["params"]["threadId"]
                .as_str()
                .ok_or_else(|| "missing thread id".to_string())?;
            let submission_id = request["params"]["clientUserMessageId"]
                .as_str()
                .ok_or_else(|| "missing client message id".to_string())?;
            let mut threads = state.threads.lock().unwrap();
            let thread = threads
                .get_mut(thread_id)
                .ok_or_else(|| "thread not found".to_string())?;
            thread.queue.push(submission_id.to_string());
            Ok(json!({
                "queuedSubmission": {
                    "id": submission_id,
                    "input": request["params"]["input"],
                    "clientUserMessageId": submission_id
                }
            }))
        }
        "turn/interrupt" => {
            let thread_id = request["params"]["threadId"]
                .as_str()
                .ok_or_else(|| "missing thread id".to_string())?;
            let turn_id = request["params"]["turnId"]
                .as_str()
                .ok_or_else(|| "missing turn id".to_string())?;
            let mut threads = state.threads.lock().unwrap();
            let thread = threads
                .get_mut(thread_id)
                .ok_or_else(|| "thread not found".to_string())?;
            if thread.active_turn.as_deref() != Some(turn_id) {
                return Err("no active turn to interrupt".to_string());
            }
            thread.active_turn = None;
            Ok(json!({}))
        }
        _ => Err(format!("unsupported method: {method}")),
    }
}

fn reconnected_queue_ids(target: &ResolvedTarget) -> Vec<String> {
    let endpoint = target.remote_endpoint.as_deref().unwrap();
    let socket_path = Path::new(endpoint.strip_prefix("unix://").unwrap());
    let mut socket = connect_unix_socket(socket_path).unwrap();
    assert!(matches!(
        request_result(
            &mut socket,
            1,
            "initialize",
            json!({"clientInfo": {"name": "test", "version": "test"}, "capabilities": {"experimentalApi": true}})
        ),
        AppServerRequestOutcome::Accepted(_)
    ));
    socket
        .send(Message::Text(
            json!({"method": "initialized"}).to_string().into(),
        ))
        .unwrap();
    let AppServerRequestOutcome::Accepted(_) = request_result(
        &mut socket,
        2,
        "thread/read",
        json!({"threadId": target.thread_id, "includeTurns": false}),
    ) else {
        panic!("thread read rejected");
    };
    let AppServerRequestOutcome::Accepted(result) = request_result(
        &mut socket,
        3,
        "thread/queue/list",
        json!({"threadId": target.thread_id, "limit": 100}),
    ) else {
        panic!("queue list rejected");
    };
    result["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn app_server_preempt_is_thread_scoped_and_queue_state_survives_reconnect() {
    let server = FakeAppServer::new([
        (
            THREAD_A,
            LiveThread {
                active_turn: Some(TURN_A.to_string()),
                queue: vec!["queued-a".to_string(), "queued-b".to_string()],
            },
        ),
        (
            THREAD_B,
            LiveThread {
                active_turn: Some(TURN_B.to_string()),
                queue: vec!["queued-b-other".to_string()],
            },
        ),
    ]);
    let target_a = server.target(THREAD_A);
    let target_b = server.target(THREAD_B);

    let first = SystemQueueControl.preempt(&target_a).unwrap();
    assert_eq!(first.current_turn_id.as_deref(), Some(TURN_A));
    assert!(first.current_turn_interrupted);
    assert_eq!(first.cancelled_submission_ids, ["queued-a", "queued-b"]);
    assert!(first.remaining_submission_ids.is_empty());
    assert!(first.session_ready);
    assert_eq!(reconnected_queue_ids(&target_a), Vec::<String>::new());
    assert_eq!(
        server
            .state
            .threads
            .lock()
            .unwrap()
            .get(THREAD_B)
            .unwrap()
            .queue,
        ["queued-b-other"]
    );

    {
        let mut threads = server.state.threads.lock().unwrap();
        let thread = threads.get_mut(THREAD_A).unwrap();
        thread.queue = vec!["idle-a".to_string(), "idle-b".to_string()];
    }
    let idle = SystemQueueControl.preempt(&target_a).unwrap();
    assert!(idle.current_turn_id.is_none());
    assert!(!idle.current_turn_interrupted);
    assert_eq!(idle.cancelled_submission_ids, ["idle-a", "idle-b"]);
    assert!(idle.session_ready);
    let repeated = SystemQueueControl.preempt(&target_a).unwrap();
    assert!(repeated.current_turn_id.is_none());
    assert!(repeated.cancelled_submission_ids.is_empty());
    assert!(repeated.session_ready);

    {
        let mut threads = server.state.threads.lock().unwrap();
        let thread = threads.get_mut(THREAD_A).unwrap();
        thread.active_turn = Some(TURN_A.to_string());
        thread.queue.clear();
    }
    let before = SystemQueueControl.queue_once(&target_a, "enqueue-before-preempt");
    let before_id = before.submission_id.clone().unwrap();
    let raced = SystemQueueControl.preempt(&target_a).unwrap();
    assert!(raced.cancelled_submission_ids.contains(&before_id));
    assert!(raced.current_turn_interrupted);

    let after = SystemQueueControl.queue_once(&target_a, "enqueue-after-boundary");
    let after_id = after.submission_id.unwrap();
    assert_eq!(
        server
            .state
            .threads
            .lock()
            .unwrap()
            .get(THREAD_A)
            .unwrap()
            .queue,
        std::slice::from_ref(&after_id)
    );
    assert_eq!(reconnected_queue_ids(&target_a), vec![after_id]);
    assert_eq!(reconnected_queue_ids(&target_b), vec!["queued-b-other"]);
}
