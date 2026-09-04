use super::session_prompt_injection::{
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
fn live_app_server_probe_requires_exact_thread_and_workspace() {
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
    let server_codex_home = codex_home.clone();
    let server_workspace = workspace.clone();
    let server = std::thread::spawn(move || {
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
            let result = match method {
                "initialize" => serde_json::json!({
                    "codexHome": server_codex_home,
                    "platformFamily": "unix",
                    "platformOs": "linux",
                    "userAgent": "probe"
                }),
                "thread/read" => serde_json::json!({
                    "thread": {
                        "id": THREAD,
                        "sessionId": THREAD,
                        "ephemeral": false,
                        "canAcceptDirectInput": true,
                        "cwd": server_workspace
                    }
                }),
                _ => continue,
            };
            socket
                .send(Message::Text(
                    serde_json::json!({"id": id, "result": result})
                        .to_string()
                        .into(),
                ))
                .unwrap();
            if method == "thread/read" {
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

    assert!(
        SystemQueueControl
            .loaded_thread_addressable(&target)
            .unwrap()
    );
    server.join().unwrap();
    let _ = std::fs::remove_dir_all(root);
}
