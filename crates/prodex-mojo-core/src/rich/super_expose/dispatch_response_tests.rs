use super::*;

#[test]
fn compiled_dispatch_response_plan_preserves_every_reply_and_audit_contract() {
    use SuperExposeDispatchDecision::*;
    let rejected = |code, message: &str| SuperExposeDispatchReply::Rejected {
        status: 400,
        code,
        message: message.into(),
    };
    for (decision, audit_required, reply) in [
        (Ok, true, SuperExposeDispatchReply::Continue),
        (
            NotificationAccepted,
            false,
            SuperExposeDispatchReply::Accepted { status: 202 },
        ),
        (
            NotificationUnsupported,
            false,
            rejected(-32601, "notification is unsupported"),
        ),
        (
            InvalidRequestId,
            false,
            rejected(-32600, "invalid request id"),
        ),
        (
            InitializeParamsRequired,
            true,
            rejected(-32602, "initialize params are required"),
        ),
        (
            ProtocolVersionRequired,
            true,
            rejected(-32602, "protocolVersion is required"),
        ),
        (
            ToolParamsRequired,
            true,
            rejected(-32602, "tool parameters are required"),
        ),
        (
            ToolNameRequired,
            true,
            rejected(-32602, "tool name is required"),
        ),
        (
            ToolArgumentsObjectRequired,
            true,
            rejected(-32602, "tool arguments must be an object"),
        ),
    ] {
        assert_eq!(
            super_expose_dispatch_response_plan(decision),
            std::result::Result::Ok(SuperExposeDispatchResponsePlan {
                audit_required,
                reply
            })
        );
    }
}

#[test]
fn dispatch_response_abi_rejects_bad_metadata_and_capacity_without_partial_output() {
    let mut output = [73_u8; 64];
    let mut metadata = [83_i64; 5];
    let mut call = |version, decision, capacity| unsafe {
        prodex_mojo_super_expose_dispatch_response_v1(
            version,
            decision,
            output.as_mut_ptr() as u64,
            capacity,
            metadata.as_mut_ptr() as u64,
        )
    };
    assert_eq!(call(2, 0, 64), 4);
    assert_eq!(call(1, -1, 64), 1);
    assert_eq!(call(1, 9, 64), 1);
    assert_eq!(call(1, 8, -1), 1);
    assert_eq!(call(1, 8, 65), 1);
    for capacity in 0.."tool arguments must be an object".len() {
        assert_eq!(call(1, 8, capacity as i64), 3);
    }
    for (address, meta) in [
        (0, metadata.as_mut_ptr() as u64),
        (output.as_mut_ptr() as u64, 0),
        (output.as_mut_ptr() as u64, metadata.as_mut_ptr() as u64 + 1),
    ] {
        assert_eq!(
            unsafe { prodex_mojo_super_expose_dispatch_response_v1(1, 8, address, 64, meta) },
            1
        );
    }
    assert_eq!(output, [73; 64]);
    assert_eq!(metadata, [83; 5]);
    let expected = b"tool arguments must be an object";
    assert_eq!(
        unsafe {
            prodex_mojo_super_expose_dispatch_response_v1(
                1,
                8,
                output.as_mut_ptr() as u64,
                expected.len() as i64,
                metadata.as_mut_ptr() as u64,
            )
        },
        0
    );
    assert_eq!(&output[..expected.len()], expected);
    assert!(output[expected.len()..].iter().all(|byte| *byte == 73));
    assert_eq!(metadata, [2, 1, 400, -32602, expected.len() as i64]);
    assert_eq!(
        unsafe {
            prodex_mojo_super_expose_dispatch_response_v1(1, 0, 0, 0, metadata.as_mut_ptr() as u64)
        },
        0
    );
    assert_eq!(metadata, [0, 1, 0, 0, 0]);
}
