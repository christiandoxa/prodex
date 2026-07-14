use runtime_proxy::RuntimeGatewayVirtualKeyRejection;

#[test]
fn missing_execution_approval_session_has_stable_fail_closed_error() {
    let rejection = RuntimeGatewayVirtualKeyRejection::GovernanceSessionRequired;
    assert_eq!(rejection.status(), 403);
    assert_eq!(rejection.code(), "governance_session_required");
}
