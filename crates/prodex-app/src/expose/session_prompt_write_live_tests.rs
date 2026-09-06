use super::session_prompt_write::{
    ProcessRecord, ProcessState, QueueControl, ResolvedTarget, SystemQueueControl,
    TargetEnvironment,
};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tungstenite::Message;

const THREAD: &str = "019f3b59-7771-7ea1-a9a1-3cd638f216c4";

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

#[test]
fn live_app_server_probe_and_turn_control_require_exact_thread_and_workspace() {
    use std::os::unix::net::UnixListener;

    let root = std::env::temp_dir().join(format!(
        "prodex-session-app-server-probe-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let codex_home = root.join("codex-home");
    let workspace = root.join("workspace");
    let socket_path = root.join(".s");
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let listener = UnixListener::bind(&socket_path).unwrap();
    let server_codex_home = codex_home.display().to_string();
    let server_workspace = workspace.display().to_string();
    let methods = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let server_methods = std::sync::Arc::clone(&methods);
    let server = std::thread::spawn(move || {
        for connection in 0..3 {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            loop {
                let Message::Text(text) = socket.read().unwrap() else {
                    continue;
                };
                let request: serde_json::Value = serde_json::from_str(text.as_ref()).unwrap();
                let Some(id) = request.get("id") else {
                    continue;
                };
                let method = request["method"].as_str().unwrap();
                server_methods.lock().unwrap().push(method.to_string());
                let result = match method {
                    "initialize" => {
                        assert_eq!(request["params"]["capabilities"]["experimentalApi"], true);
                        assert!(
                            !request["params"]["clientInfo"]["version"]
                                .as_str()
                                .unwrap_or_default()
                                .is_empty()
                        );
                        serde_json::json!({
                            "codexHome": server_codex_home.clone(),
                            "platformFamily": "unix",
                            "platformOs": "linux",
                            "userAgent": "probe"
                        })
                    }
                    "thread/read" => serde_json::json!({
                        "thread": if connection == 2 {
                            serde_json::json!({
                                "id": THREAD,
                                "sessionId": THREAD,
                                "ephemeral": false,
                                "cwd": server_workspace.clone(),
                                "status": {"type": "notLoaded"}
                            })
                        } else {
                            serde_json::json!({
                                "id": THREAD,
                                "sessionId": THREAD,
                                "ephemeral": false,
                                "canAcceptDirectInput": true,
                                "cwd": server_workspace.clone(),
                                "status": {"type": "idle"}
                            })
                        }
                    }),
                    "thread/queue/add" => {
                        assert_eq!(request["params"]["threadId"], THREAD);
                        assert_eq!(request["params"]["input"][0]["type"], "text");
                        assert_eq!(request["params"]["input"][0]["text"], "visible message");
                        let client_id = request["params"]["clientUserMessageId"]
                            .as_str()
                            .unwrap_or_default();
                        assert!(uuid::Uuid::parse_str(client_id).is_ok());
                        serde_json::json!({
                            "queuedSubmission": {
                                "id": "019f3b59-7771-7ea1-a9a1-3cd638f216c5",
                                "clientUserMessageId": client_id,
                                "input": request["params"]["input"].clone()
                            }
                        })
                    }
                    _ => continue,
                };
                socket
                    .send(Message::Text(
                        serde_json::json!({"id": id, "result": result})
                            .to_string()
                            .into(),
                    ))
                    .unwrap();
                if method == "thread/queue/add"
                    || (connection == 0 && method == "thread/read")
                    || (connection == 2 && method == "thread/read")
                {
                    break;
                }
            }
        }
    });
    let endpoint = format!("unix://{}", socket_path.display());
    let target = ResolvedTarget {
        prodex: process(
            100,
            1,
            "/usr/bin/prodex",
            vec!["prodex", "s"],
            &workspace,
            10,
        ),
        writer: process(
            200,
            100,
            "/usr/bin/codex",
            vec!["codex", "app-server", "--listen", &endpoint],
            &workspace,
            20,
        ),
        thread_id: THREAD.to_string(),
        queue_db: root.join("queue_1.sqlite"),
        state_db: root.join("state_5.sqlite"),
        environment: TargetEnvironment {
            home: "/home/test-user".to_string(),
            codex_home,
            codex_sqlite_home: root.clone(),
            pwd: workspace.display().to_string(),
        },
        remote_endpoint: Some(endpoint),
    };

    assert!(
        SystemQueueControl
            .loaded_thread_addressable(&target)
            .unwrap()
    );
    let invocation = SystemQueueControl.queue_once(&target, "visible message");
    assert!(
        invocation.outcome == super::session_prompt_write::QueueRequestOutcome::Accepted,
        "app-server methods: {:?}",
        methods.lock().unwrap()
    );
    assert!(invocation.message_id.is_some());
    assert_eq!(
        methods.lock().unwrap().as_slice(),
        [
            "initialize",
            "thread/read",
            "initialize",
            "thread/read",
            "thread/queue/add"
        ]
    );
    assert!(
        !SystemQueueControl
            .loaded_thread_addressable(&target)
            .unwrap()
    );
    assert_eq!(
        methods.lock().unwrap().as_slice(),
        [
            "initialize",
            "thread/read",
            "initialize",
            "thread/read",
            "thread/queue/add",
            "initialize",
            "thread/read"
        ]
    );
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn live_app_server_busy_prompt_write_uses_authoritative_queue() {
    use std::os::unix::net::UnixListener;

    let root = std::env::temp_dir().join(format!(
        "prodex-session-app-server-queue-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let codex_home = root.join("codex-home");
    let workspace = root.join("workspace");
    let socket_path = root.join(".s");
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::create_dir_all(&workspace).unwrap();
    let listener = UnixListener::bind(&socket_path).unwrap();
    let server_codex_home = codex_home.display().to_string();
    let server_workspace = workspace.display().to_string();
    let expected_message = "\\".repeat(32 * 1024);
    let server_expected_message = expected_message.clone();
    let methods = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let server_methods = std::sync::Arc::clone(&methods);
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        let mut initialize_seen = false;
        let mut initialized = false;
        loop {
            let Message::Text(text) = socket.read().unwrap() else {
                continue;
            };
            let request: serde_json::Value = serde_json::from_str(text.as_ref()).unwrap();
            let method = request["method"].as_str().unwrap();
            if request.get("id").is_none() {
                if method == "initialized" && initialize_seen {
                    initialized = true;
                }
                continue;
            }
            let Some(id) = request.get("id") else {
                continue;
            };
            server_methods.lock().unwrap().push(method.to_string());
            if method != "initialize" && !initialized {
                socket
                    .send(Message::Text(
                        serde_json::json!({
                            "id": id,
                            "error": {"code": -32600, "message": "Not initialized"}
                        })
                        .to_string()
                        .into(),
                    ))
                    .unwrap();
                continue;
            }
            let result = match method {
                "initialize" => {
                    initialize_seen = true;
                    serde_json::json!({
                        "codexHome": server_codex_home.clone(),
                        "platformFamily": "unix",
                        "platformOs": "linux",
                        "userAgent": "probe"
                    })
                }
                "thread/read" => serde_json::json!({
                    "thread": {
                        "id": THREAD,
                        "sessionId": THREAD,
                        "ephemeral": false,
                        "canAcceptDirectInput": true,
                        "cwd": server_workspace.clone(),
                        "status": {"type": "active", "activeFlags": []}
                    }
                }),
                "thread/queue/add" => {
                    assert_eq!(request["params"]["threadId"], THREAD);
                    assert_eq!(request["params"]["input"][0]["type"], "text");
                    assert_eq!(
                        request["params"]["input"][0]["text"],
                        server_expected_message
                    );
                    let client_id = request["params"]["clientUserMessageId"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    serde_json::json!({
                        "queuedSubmission": {
                            "id": "019f3b59-7771-7ea1-a9a1-3cd638f216c6",
                            "clientUserMessageId": client_id,
                            "input": request["params"]["input"].clone()
                        }
                    })
                }
                _ => continue,
            };
            socket
                .send(Message::Text(
                    serde_json::json!({"id": id, "result": result})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            if method == "thread/queue/add" {
                break;
            }
        }
    });
    let endpoint = format!("unix://{}", socket_path.display());
    let target = ResolvedTarget {
        prodex: process(
            100,
            1,
            "/usr/bin/prodex",
            vec!["prodex", "s"],
            &workspace,
            10,
        ),
        writer: process(
            200,
            100,
            "/usr/bin/codex",
            vec!["codex", "app-server", "--listen", &endpoint],
            &workspace,
            20,
        ),
        thread_id: THREAD.to_string(),
        queue_db: root.join("queue_1.sqlite"),
        state_db: root.join("state_5.sqlite"),
        environment: TargetEnvironment {
            home: "/home/test-user".to_string(),
            codex_home,
            codex_sqlite_home: root.clone(),
            pwd: workspace.display().to_string(),
        },
        remote_endpoint: Some(endpoint),
    };

    let invocation = SystemQueueControl.queue_once(&target, &expected_message);
    server.join().unwrap();
    assert_eq!(
        invocation.outcome,
        super::session_prompt_write::QueueRequestOutcome::Accepted
    );
    assert!(invocation.queued);
    assert!(invocation.message_id.is_some());
    assert_eq!(
        invocation.submission_id.as_deref(),
        Some("019f3b59-7771-7ea1-a9a1-3cd638f216c6")
    );
    assert_eq!(
        methods.lock().unwrap().as_slice(),
        ["initialize", "thread/read", "thread/queue/add"]
    );
    let _ = std::fs::remove_dir_all(root);
}
