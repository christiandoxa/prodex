#![cfg(feature = "mojo-core")]

use prodex_mojo_core::rich::{KiroKernelInput, KiroKernelOperation, kiro_kernel};

#[test]
fn final_event_plan_selects_exact_json_bytes() {
    for (status, event_type, response) in [
        (
            Some("failed"),
            "response.failed",
            r#"{"id":"resp_1","status":"failed"}"#,
        ),
        (
            Some("incomplete"),
            "response.incomplete",
            r#"{"id":"resp_1","status":"incomplete"}"#,
        ),
        (
            Some("queued"),
            "response.completed",
            r#"{"id":"resp_1","status":"queued"}"#,
        ),
        (None, "response.completed", r#"{"id":"resp_1"}"#),
    ] {
        let mut input = KiroKernelInput::new(KiroKernelOperation::ResponseFinalEvent);
        input.sequence_number = 7;
        input.created_at = 123;
        input.status = status;
        input.output = Some(response);

        let expected = format!(
            r#"{{"type":"{event_type}","sequence_number":7,"created_at":123,"response":{response}}}"#
        );
        assert_eq!(
            kiro_kernel(input).expect("real Kiro Mojo kernel succeeds"),
            expected.as_bytes()
        );
    }
}
