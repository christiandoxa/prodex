//! Recover an idle native thread through its owned app-server, without ending
//! the frontend, reloading an overlay, or replaying terminal keyboard input.
use anyhow::Result;
use prodex_mojo_core::launch::{
    InPlaceRecoveryAction, NativeGoalStatus, NativeThreadStatus, in_place_recovery_action,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tungstenite::Message;

use crate::super_expose::app_server_control::{
    AppServerRequestOutcome, UnixAppServerSocket, connect_unix_socket, request_result,
};

#[derive(Clone)]
pub(super) struct NativeSessionRecovery {
    socket: PathBuf,
    codex_home: PathBuf,
    sqlite_home: PathBuf,
    workspace: PathBuf,
    affinity: Option<crate::RuntimeBrokerSessionAffinityControl>,
    next_attempt_at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeRecoveryOutcome {
    Deferred,
    Stopped,
    Accepted,
    Ambiguous,
}

impl NativeSessionRecovery {
    pub(super) fn new(
        socket: PathBuf,
        codex_home: PathBuf,
        sqlite_home: PathBuf,
        workspace: PathBuf,
        affinity: Option<crate::RuntimeBrokerSessionAffinityControl>,
    ) -> Self {
        Self {
            socket,
            codex_home,
            sqlite_home,
            workspace,
            affinity,
            next_attempt_at: Instant::now(),
        }
    }

    pub(super) fn recover(
        &mut self,
        session_id: &str,
        expected_goal: bool,
    ) -> NativeRecoveryOutcome {
        if Instant::now() < self.next_attempt_at {
            return NativeRecoveryOutcome::Deferred;
        }
        self.next_attempt_at = Instant::now() + Duration::from_secs(5);
        if uuid::Uuid::parse_str(session_id).is_err() {
            return NativeRecoveryOutcome::Stopped;
        }
        let Ok(mut socket) = connect_unix_socket(&self.socket) else {
            return NativeRecoveryOutcome::Deferred;
        };
        let workspace = crate::runtime_thread_index::runtime_thread_workspace_for_session(
            &self.sqlite_home,
            session_id,
        )
        .unwrap_or_else(|| self.workspace.clone());
        recover_on_socket(
            &mut socket,
            &self.codex_home,
            &workspace,
            session_id,
            expected_goal,
            || {
                if let Some(control) = self.affinity.as_ref() {
                    // Release this thread's session/compact preference only.
                    // Existing response-ID/turn-state hard ownership is retained.
                    crate::runtime_broker::release_runtime_broker_session_affinity(
                        &control.client,
                        &control.paths,
                        &control.broker_key,
                        &control.registry,
                        session_id,
                    )?;
                }
                Ok(())
            },
        )
    }
}

fn accepted(outcome: AppServerRequestOutcome) -> Option<Value> {
    match outcome {
        AppServerRequestOutcome::Accepted(value) => Some(value),
        AppServerRequestOutcome::Rejected | AppServerRequestOutcome::Ambiguous => None,
    }
}

fn native_goal_status(goal: Option<&Value>) -> NativeGoalStatus {
    match goal
        .and_then(|goal| goal.get("status"))
        .and_then(Value::as_str)
    {
        Some("active") => NativeGoalStatus::Active,
        Some("blocked") => NativeGoalStatus::Blocked,
        Some("usageLimited") => NativeGoalStatus::UsageLimited,
        Some("paused") => NativeGoalStatus::Paused,
        Some("budgetLimited") => NativeGoalStatus::BudgetLimited,
        Some("complete") => NativeGoalStatus::Complete,
        None if goal.is_none() => NativeGoalStatus::Absent,
        _ => NativeGoalStatus::Unknown,
    }
}

/// Project the native wire status after verifying that this is our loaded,
/// persistent workspace thread. Mojo owns quiescent-state recovery eligibility.
fn verified_native_thread_status(
    thread: &Value,
    workspace: &Path,
    session_id: &str,
) -> Result<NativeThreadStatus, NativeRecoveryOutcome> {
    let identity = thread.get("id").and_then(Value::as_str) == Some(session_id)
        && thread.get("sessionId").and_then(Value::as_str) == Some(session_id)
        && thread.get("ephemeral").and_then(Value::as_bool) == Some(false)
        && thread.get("canAcceptDirectInput").and_then(Value::as_bool) == Some(true)
        && thread
            .get("cwd")
            .and_then(Value::as_str)
            .is_some_and(|cwd| prodex_core::same_path(Path::new(cwd), workspace));
    if !identity {
        return Err(NativeRecoveryOutcome::Stopped);
    }
    Ok(
        match thread
            .get("status")
            .and_then(|status| status.get("type"))
            .and_then(Value::as_str)
        {
            Some("idle") => NativeThreadStatus::Idle,
            Some("systemError") => NativeThreadStatus::SystemError,
            Some("active") => NativeThreadStatus::Active,
            Some("notLoaded") => NativeThreadStatus::NotLoaded,
            _ => NativeThreadStatus::Unknown,
        },
    )
}

fn native_goal_budget_available(goal: Option<&Value>) -> bool {
    goal.is_none_or(|goal| match goal.get("tokenBudget") {
        None | Some(Value::Null) => true,
        Some(budget) => budget
            .as_i64()
            .zip(goal.get("tokensUsed").and_then(Value::as_i64))
            .is_some_and(|(limit, used)| limit > 0 && used >= 0 && used < limit),
    })
}

fn read_native_recovery_plan(
    socket: &mut UnixAppServerSocket,
    codex_home: &Path,
    workspace: &Path,
    session_id: &str,
    expected_goal: bool,
) -> Result<InPlaceRecoveryAction, NativeRecoveryOutcome> {
    let initialized = accepted(request_result(
        socket,
        1,
        "initialize",
        json!({
            "clientInfo":{"name":"prodex-native-recovery", "version":env!("CARGO_PKG_VERSION")},
            "capabilities":{"experimentalApi":true}
        }),
    ))
    .ok_or(NativeRecoveryOutcome::Deferred)?;
    if !initialized
        .get("codexHome")
        .and_then(Value::as_str)
        .is_some_and(|home| prodex_core::same_path(Path::new(home), codex_home))
    {
        return Err(NativeRecoveryOutcome::Stopped);
    }
    socket
        .send(Message::Text(
            json!({"method":"initialized"}).to_string().into(),
        ))
        .map_err(|_| NativeRecoveryOutcome::Deferred)?;
    let read = accepted(request_result(
        socket,
        2,
        "thread/read",
        json!({
            "threadId":session_id, "includeTurns":false
        }),
    ))
    .ok_or(NativeRecoveryOutcome::Deferred)?;
    let thread = read.get("thread").ok_or(NativeRecoveryOutcome::Deferred)?;
    let thread_status = verified_native_thread_status(thread, workspace, session_id)?;
    let goals = accepted(request_result(
        socket,
        3,
        "thread/goal/get",
        json!({
            "threadId":session_id
        }),
    ))
    .ok_or(NativeRecoveryOutcome::Deferred)?;
    // Missing/malformed envelopes are not evidence that a saved goal was removed.
    let goal_field = goals.get("goal").ok_or(NativeRecoveryOutcome::Deferred)?;
    let goal = (!goal_field.is_null()).then_some(goal_field);
    if goal.is_some_and(|goal| goal.get("threadId").and_then(Value::as_str) != Some(session_id)) {
        return Err(NativeRecoveryOutcome::Stopped);
    }
    in_place_recovery_action(
        true,
        thread_status,
        expected_goal,
        native_goal_status(goal),
        native_goal_budget_available(goal),
    )
    .map_err(|_| NativeRecoveryOutcome::Stopped)
}

fn native_mutation_confirmed(
    action: InPlaceRecoveryAction,
    result: &Value,
    session_id: &str,
) -> bool {
    match action {
        InPlaceRecoveryAction::ResumeGoal => result.get("goal").is_some_and(|goal| {
            goal.get("threadId").and_then(Value::as_str) == Some(session_id)
                && goal.get("status").and_then(Value::as_str) == Some("active")
        }),
        InPlaceRecoveryAction::ContinueTurn => result.get("turn").is_some_and(|turn| {
            turn.get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty())
        }),
        _ => false,
    }
}

fn recover_on_socket(
    socket: &mut UnixAppServerSocket,
    codex_home: &Path,
    workspace: &Path,
    session_id: &str,
    expected_goal: bool,
    release_affinity: impl FnOnce() -> Result<()>,
) -> NativeRecoveryOutcome {
    let action =
        match read_native_recovery_plan(socket, codex_home, workspace, session_id, expected_goal) {
            Ok(action) => action,
            Err(outcome) => return outcome,
        };
    let (method, params) = match action {
        InPlaceRecoveryAction::Defer => return NativeRecoveryOutcome::Deferred,
        InPlaceRecoveryAction::Stop => return NativeRecoveryOutcome::Stopped,
        InPlaceRecoveryAction::ResumeGoal => (
            "thread/goal/set",
            json!({
                "threadId":session_id, "status":"active", "origin":"automatic"
            }),
        ),
        InPlaceRecoveryAction::ContinueTurn => (
            "turn/start",
            json!({
                "threadId":session_id,
                "input":[{"type":"text", "text":crate::app_commands::runtime_launch::RUNTIME_SESSION_CONTINUATION_PROMPT}]
            }),
        ),
    };
    if release_affinity().is_err() {
        return NativeRecoveryOutcome::Deferred;
    }
    match request_result(socket, 4, method, params) {
        AppServerRequestOutcome::Accepted(result) => {
            if native_mutation_confirmed(action, &result, session_id) {
                NativeRecoveryOutcome::Accepted
            } else {
                NativeRecoveryOutcome::Ambiguous
            }
        }
        // Preserve native rejection and never duplicate a possibly accepted RPC.
        AppServerRequestOutcome::Rejected => NativeRecoveryOutcome::Stopped,
        AppServerRequestOutcome::Ambiguous => NativeRecoveryOutcome::Ambiguous,
    }
}

#[cfg(test)]
#[path = "session_recovery_tests.rs"]
mod tests;
