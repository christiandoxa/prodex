use super::*;
use std::os::unix::net::UnixListener;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use tungstenite::accept;

const THREAD: &str = "00000000-0000-7000-8000-000000000042";

#[derive(Clone, Copy)]
struct Scenario {
    status: &'static str,
    goal: Option<&'static str>,
    expected_goal: bool,
    exhausted_budget: bool,
    wrong_thread: bool,
    wrong_home: bool,
    reject_mutation: bool,
    ambiguous_mutation: bool,
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            status: "idle",
            goal: Some("blocked"),
            expected_goal: true,
            exhausted_budget: false,
            wrong_thread: false,
            wrong_home: false,
            reject_mutation: false,
            ambiguous_mutation: false,
        }
    }
}

fn mock_recovery(scenario: Scenario) -> (NativeRecoveryOutcome, Vec<Value>, bool) {
    let root = std::env::temp_dir().join(format!(
        "prodex-native-recovery-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("native.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let home = root.clone();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let mut socket = accept(stream).unwrap();
        let mut calls = Vec::new();
        while let Ok(message) = socket.read() {
            let Message::Text(text) = message else {
                continue;
            };
            let request: Value = serde_json::from_str(&text).unwrap();
            let method = request["method"].as_str().unwrap().to_string();
            calls.push(request.clone());
            if method == "initialized" {
                continue;
            }
            let result = match method.as_str() {
                "initialize" => {
                    json!({"codexHome":if scenario.wrong_home { home.join("other") } else { home.clone() }})
                }
                "thread/read" => json!({"thread":{
                    "id":if scenario.wrong_thread { "another-thread" } else { THREAD },
                    "sessionId":THREAD, "ephemeral":false, "canAcceptDirectInput":true,
                    "cwd":home, "status":{"type":scenario.status}
                }}),
                "thread/goal/get" => json!({"goal":scenario.goal.map(|status| json!({
                    "threadId":THREAD, "status":status, "objective":"preserved synthetic objective",
                    "tokenBudget":100, "tokensUsed":if scenario.exhausted_budget {100} else {10}
                }))}),
                "thread/goal/set" | "turn/start" => {
                    if scenario.ambiguous_mutation {
                        let _ = socket.close(None);
                        break;
                    }
                    if scenario.reject_mutation {
                        socket.send(Message::Text(json!({"id":request["id"],"error":{"code":-32602,"message":"native policy rejected"}}).to_string().into())).unwrap();
                        break;
                    }
                    if method == "thread/goal/set" {
                        json!({"goal":{"threadId":THREAD,"status":"active"}})
                    } else {
                        json!({"turn":{"id":"native-continuation-turn","status":"inProgress"}})
                    }
                }
                other => panic!("unexpected native recovery method {other}"),
            };
            socket
                .send(Message::Text(
                    json!({"id":request["id"],"result":result})
                        .to_string()
                        .into(),
                ))
                .unwrap();
        }
        calls
    });
    let released = Arc::new(AtomicBool::new(false));
    let release_record = Arc::clone(&released);
    let mut socket = connect_unix_socket(&path).unwrap();
    let outcome = recover_on_socket(
        &mut socket,
        &root,
        &root,
        THREAD,
        scenario.expected_goal,
        move || {
            release_record.store(true, Ordering::SeqCst);
            Ok(())
        },
    );
    drop(socket);
    let calls = server.join().unwrap();
    let was_released = released.load(Ordering::SeqCst);
    let _ = std::fs::remove_dir_all(root);
    (outcome, calls, was_released)
}

fn mutations(calls: &[Value]) -> Vec<&Value> {
    calls
        .iter()
        .filter(|call| {
            matches!(
                call["method"].as_str(),
                Some("thread/goal/set" | "turn/start")
            )
        })
        .collect()
}

#[test]
fn native_goal_recovery_uses_same_thread_without_reopening_or_keyboard_resume() {
    let (outcome, calls, released) = mock_recovery(Scenario::default());
    assert_eq!(outcome, NativeRecoveryOutcome::Accepted);
    assert!(released);
    let writes = mutations(&calls);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0]["method"], "thread/goal/set");
    assert_eq!(
        writes[0]["params"],
        json!({"threadId":THREAD,"status":"active","origin":"automatic"})
    );
    assert!(!calls.iter().any(|call| call["method"] == "thread/resume"));
    assert!(
        !serde_json::to_string(&calls)
            .unwrap()
            .contains("/goal resume")
    );
}

#[test]
fn native_active_turn_is_not_interrupted_or_rotated() {
    let (outcome, calls, released) = mock_recovery(Scenario {
        status: "active",
        ..Scenario::default()
    });
    assert_eq!(outcome, NativeRecoveryOutcome::Deferred);
    assert!(!released);
    assert!(mutations(&calls).is_empty());
}

#[test]
fn native_goal_recovery_respects_user_stops_budgets_and_removed_goals() {
    for goal in [
        Some("paused"),
        Some("budgetLimited"),
        Some("complete"),
        Some("unrecognized"),
        None,
    ] {
        let (outcome, calls, released) = mock_recovery(Scenario {
            goal,
            ..Scenario::default()
        });
        assert_eq!(outcome, NativeRecoveryOutcome::Stopped, "{goal:?}");
        assert!(!released);
        assert!(mutations(&calls).is_empty());
    }
    let (outcome, calls, released) = mock_recovery(Scenario {
        exhausted_budget: true,
        ..Scenario::default()
    });
    assert_eq!(outcome, NativeRecoveryOutcome::Stopped);
    assert!(!released);
    assert!(mutations(&calls).is_empty());
}

#[test]
fn native_goal_recovery_refuses_other_homes_and_threads() {
    for scenario in [
        Scenario {
            wrong_home: true,
            ..Scenario::default()
        },
        Scenario {
            wrong_thread: true,
            ..Scenario::default()
        },
    ] {
        let (outcome, calls, released) = mock_recovery(scenario);
        assert_eq!(outcome, NativeRecoveryOutcome::Stopped);
        assert!(!released);
        assert!(mutations(&calls).is_empty());
    }
}

#[test]
fn native_non_goal_continuation_does_not_resend_the_original_prompt() {
    let (outcome, calls, released) = mock_recovery(Scenario {
        goal: None,
        expected_goal: false,
        ..Scenario::default()
    });
    assert_eq!(outcome, NativeRecoveryOutcome::Accepted);
    assert!(released);
    let writes = mutations(&calls);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0]["method"], "turn/start");
    assert_eq!(
        writes[0]["params"]["input"][0]["text"],
        crate::app_commands::runtime_launch::RUNTIME_SESSION_CONTINUATION_PROMPT
    );
}

#[test]
fn native_ambiguous_delivery_and_rejection_never_duplicate_the_mutation() {
    for (scenario, expected) in [
        (
            Scenario {
                ambiguous_mutation: true,
                ..Scenario::default()
            },
            NativeRecoveryOutcome::Ambiguous,
        ),
        (
            Scenario {
                reject_mutation: true,
                ..Scenario::default()
            },
            NativeRecoveryOutcome::Stopped,
        ),
    ] {
        let (outcome, calls, _) = mock_recovery(scenario);
        assert_eq!(outcome, expected);
        assert_eq!(mutations(&calls).len(), 1);
    }
}
