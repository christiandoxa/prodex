use super::session_prompt_write::{
    ProcessRecord, ProcessState, QueueControl, ResolvedTarget, SystemQueueControl,
    TargetEnvironment,
};
use crate::app_server_control::{AppServerRequestOutcome, connect_unix_socket, request_result};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, UNIX_EPOCH};
use tungstenite::Message;

#[path = "session_preempt_live_edge_tests.rs"]
mod edge;

const THREAD_A: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216c4";
const THREAD_B: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216d4";
const TURN_A: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216e4";
const TURN_B: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216f4";

#[derive(Clone)]
struct LiveThread {
    active_turn: Option<String>,
    queue: Vec<String>,
}

#[derive(Default)]
struct LiveServerState {
    threads: Mutex<BTreeMap<String, LiveThread>>,
    requests: Mutex<Vec<String>>,
    thread_read_include_turns: Mutex<Vec<bool>>,
    malformed_queue: AtomicBool,
    interrupt_rejected: AtomicBool,
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
            requests: Mutex::new(Vec::new()),
            thread_read_include_turns: Mutex::new(Vec::new()),
            malformed_queue: AtomicBool::new(false),
            interrupt_rejected: AtomicBool::new(false),
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
    state.requests.lock().unwrap().push(method.to_string());
    if method == "thread/queue/list" && state.malformed_queue.load(Ordering::SeqCst) {
        return Ok(json!({"data": [{"id": 7}], "nextCursor": null}));
    }
    if method == "turn/interrupt" && state.interrupt_rejected.load(Ordering::SeqCst) {
        return Err("turn changed before interrupt".to_string());
    }
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
            let include_turns = request["params"]["includeTurns"].as_bool().unwrap_or(false);
            state
                .thread_read_include_turns
                .lock()
                .unwrap()
                .push(include_turns);
            Ok(json!({
                "thread": {
                    "id": thread_id,
                    "sessionId": thread_id,
                    "ephemeral": false,
                    "canAcceptDirectInput": true,
                    "cwd": workspace,
                    "status": if thread.active_turn.is_some() { json!({"type": "active", "activeFlags": []}) } else { json!({"type": "idle"}) },
                    "turns": if include_turns {
                        thread
                            .active_turn
                            .as_ref()
                            .map(|id| vec![json!({"id": id, "status": {"type": "inProgress"}})])
                            .unwrap_or_default()
                    } else {
                        Vec::<Value>::new()
                    }
                }
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
    assert_eq!(
        server
            .state
            .thread_read_include_turns
            .lock()
            .unwrap()
            .as_slice(),
        [true, true, true]
    );
    let methods = server.state.requests.lock().unwrap().clone();
    let interrupt_index = methods
        .iter()
        .position(|method| method == "turn/interrupt")
        .unwrap();
    let last_queue_list_before_interrupt = methods[..interrupt_index]
        .iter()
        .rposition(|method| method == "thread/queue/list")
        .unwrap();
    assert!(last_queue_list_before_interrupt < interrupt_index);
    assert!(!methods.iter().any(|method| method == "thread/turns/list"));
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
