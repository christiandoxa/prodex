use super::*;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeProxyBackendFaultRoute {
    Responses,
    Compact,
    Realtime,
    Status,
    Usage,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeProxyBackendFaultStep {
    route: RuntimeProxyBackendFaultRoute,
    account_id: Option<String>,
    status_line: &'static str,
    content_type: &'static str,
    body: String,
    response_turn_state: Option<String>,
    initial_body_stall: Option<Duration>,
    chunk_delay: Option<Duration>,
}

impl RuntimeProxyBackendFaultStep {
    pub(crate) fn sse_quota(route: RuntimeProxyBackendFaultRoute, account_id: &str) -> Self {
        Self {
            route,
            account_id: Some(account_id.to_string()),
            status_line: "HTTP/1.1 200 OK",
            content_type: "text/event-stream",
            body: concat!(
                "event: response.failed\r\n",
                "data: {\"type\":\"response.failed\",\"response\":{\"error\":{\"code\":\"insufficient_quota\",\"message\":\"scripted quota exhausted\"}}}\r\n",
                "\r\n"
            )
            .to_string(),
            response_turn_state: None,
            initial_body_stall: None,
            chunk_delay: None,
        }
    }

    pub(crate) fn plain_429(route: RuntimeProxyBackendFaultRoute, account_id: &str) -> Self {
        Self {
            route,
            account_id: Some(account_id.to_string()),
            status_line: "HTTP/1.1 429 Too Many Requests",
            content_type: "text/plain",
            body: "Too Many Requests".to_string(),
            response_turn_state: None,
            initial_body_stall: None,
            chunk_delay: None,
        }
    }

    pub(crate) fn workspace_credits_exhausted(
        route: RuntimeProxyBackendFaultRoute,
        account_id: &str,
    ) -> Self {
        Self {
            route,
            account_id: Some(account_id.to_string()),
            status_line: "HTTP/1.1 429 Too Many Requests",
            content_type: "application/json",
            body: serde_json::json!({
                "error": {
                    "message": "Your workspace is out of credits. Ask your workspace owner to refill in order to continue."
                }
            })
            .to_string(),
            response_turn_state: None,
            initial_body_stall: None,
            chunk_delay: None,
        }
    }

    pub(crate) fn stalled_json(
        route: RuntimeProxyBackendFaultRoute,
        account_id: &str,
        stall: Duration,
    ) -> Self {
        Self {
            route,
            account_id: Some(account_id.to_string()),
            status_line: "HTTP/1.1 200 OK",
            content_type: "application/json",
            body: serde_json::json!({ "ok": true }).to_string(),
            response_turn_state: None,
            initial_body_stall: Some(stall),
            chunk_delay: None,
        }
    }

    fn matches(&self, route: RuntimeProxyBackendFaultRoute, account_id: &str) -> bool {
        self.route == route && self.account_id.as_deref().is_none_or(|id| id == account_id)
    }

    fn into_response(self) -> RuntimeProxyBackendHttpResponse {
        RuntimeProxyBackendHttpResponse::new(
            self.status_line,
            self.content_type,
            self.body,
            self.response_turn_state,
            self.initial_body_stall,
            self.chunk_delay,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeProxyBackendFaultScript {
    steps: VecDeque<RuntimeProxyBackendFaultStep>,
}

impl RuntimeProxyBackendFaultScript {
    pub(crate) fn new(steps: impl IntoIterator<Item = RuntimeProxyBackendFaultStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
        }
    }

    pub(crate) fn next_response(
        &mut self,
        route: RuntimeProxyBackendFaultRoute,
        account_id: &str,
    ) -> Option<RuntimeProxyBackendHttpResponse> {
        let index = self
            .steps
            .iter()
            .position(|step| step.matches(route, account_id))?;
        self.steps.remove(index).map(RuntimeProxyBackendFaultStep::into_response)
    }
}

pub(crate) fn runtime_proxy_backend_fault_route_for_path(
    path: &str,
) -> Option<RuntimeProxyBackendFaultRoute> {
    if path.ends_with("/backend-api/codex/responses") {
        Some(RuntimeProxyBackendFaultRoute::Responses)
    } else if path.ends_with("/backend-api/codex/responses/compact") {
        Some(RuntimeProxyBackendFaultRoute::Compact)
    } else if path.ends_with("/backend-api/codex/realtime/calls") {
        Some(RuntimeProxyBackendFaultRoute::Realtime)
    } else if path.ends_with("/backend-api/status") {
        Some(RuntimeProxyBackendFaultRoute::Status)
    } else if path.ends_with("/backend-api/wham/usage") {
        Some(RuntimeProxyBackendFaultRoute::Usage)
    } else {
        None
    }
}
