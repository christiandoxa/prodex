use crate::{
    RuntimeHttpErrorPhase, RuntimeSseInspectionProgress, RuntimeWebsocketRetryInspectionKind,
    inspect_runtime_sse_buffer, inspect_runtime_sse_buffer_at_eof,
    inspect_runtime_websocket_text_frame_with_phase,
};

const PRELUDE: &str = concat!(
    "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-held\"}}\r\n\r\n",
    "data: {\"type\":\"codex.rate_limits\",\"rate_limits\":[]}\r\n\r\n",
    "data: {\"type\":\"keepalive\"}\r\n\r\n",
);
const OVERLOAD: &str = concat!(
    "data: {\"type\":\"response.failed\",\"response\":{\"error\":{",
    "\"code\":\"server_is_overloaded\",\"message\":\"Selected model is at capacity. Please try a different model.\"}}}\r\n\r\n",
);

#[test]
fn keepalive_sse_is_not_a_model_output_commit() {
    assert_eq!(
        inspect_runtime_sse_buffer(PRELUDE.as_bytes()),
        RuntimeSseInspectionProgress::Hold {
            response_ids: vec!["resp-held".to_string()],
            turn_state: None,
        },
        "heartbeats must not pin a failed attempt or expose it to the client"
    );
}

#[test]
fn keepalive_sse_then_capacity_error_remains_retryable_at_all_chunk_boundaries() {
    let body = format!("{PRELUDE}{OVERLOAD}");
    for split in 0..=body.len() {
        let first = inspect_runtime_sse_buffer(&body.as_bytes()[..split]);
        assert!(
            !matches!(first, RuntimeSseInspectionProgress::Commit { .. }),
            "metadata, heartbeat or partial error committed at split {split}: {first:?}"
        );
    }
    assert_eq!(
        inspect_runtime_sse_buffer(body.as_bytes()),
        RuntimeSseInspectionProgress::Overloaded,
    );
    assert_eq!(
        inspect_runtime_sse_buffer_at_eof(body.trim_end().as_bytes()),
        RuntimeSseInspectionProgress::Overloaded,
    );
}

#[test]
fn keepalive_does_not_allow_retry_after_real_text_or_tool_output() {
    for output in [
        r#"{"type":"response.output_text.delta","delta":"already visible"}"#,
        r#"{"type":"response.function_call_arguments.delta","delta":"{"}"#,
        r#"{"type":"response.reasoning_summary_text.delta","delta":"already visible"}"#,
    ] {
        let body = format!("{PRELUDE}data: {output}\r\n\r\n{OVERLOAD}");
        assert!(
            matches!(
                inspect_runtime_sse_buffer(body.as_bytes()),
                RuntimeSseInspectionProgress::Commit { .. }
            ),
            "a heartbeat must not revoke a real output commit: {output}"
        );
    }
}

#[test]
fn keepalive_websocket_holds_until_a_retryable_capacity_frame_or_real_output() {
    let heartbeat = inspect_runtime_websocket_text_frame_with_phase(
        r#"{"type":"keepalive"}"#,
        RuntimeHttpErrorPhase::PreCommit,
    );
    assert!(heartbeat.precommit_hold);
    assert!(!heartbeat.terminal_event);
    assert_eq!(heartbeat.retry_kind, None);
    let failure = r#"{"type":"response.failed","response":{"error":{"code":"server_is_overloaded","message":"Selected model is at capacity. Please try a different model."}}}"#;
    assert_eq!(
        inspect_runtime_websocket_text_frame_with_phase(failure, RuntimeHttpErrorPhase::PreCommit)
            .retry_kind,
        Some(RuntimeWebsocketRetryInspectionKind::Overloaded),
    );
    assert_eq!(
        inspect_runtime_websocket_text_frame_with_phase(failure, RuntimeHttpErrorPhase::Committed)
            .retry_kind,
        None,
    );
    let completed = inspect_runtime_websocket_text_frame_with_phase(
        r#"{"type":"response.completed","response":{"id":"resp-completed"}}"#,
        RuntimeHttpErrorPhase::PreCommit,
    );
    assert!(!completed.precommit_hold);
    assert!(completed.terminal_event);
}
