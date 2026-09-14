use super::*;

impl fmt::Debug for ApplicationRequestDeadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ApplicationRequestDeadline")
            .field(&"<redacted>")
            .finish()
    }
}

impl fmt::Debug for ApplicationRequestMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestMetadata")
            .field("observed_header_count", &self.observed_header_count)
            .field("headers_truncated", &self.headers_truncated)
            .field("trace_context_present", &self.trace_context_present)
            .field("credential_present", &self.credential_present)
            .field("affinity_present", &self.affinity_present)
            .field("codex_metadata_present", &self.codex_metadata_present)
            .field("user_agent_present", &self.user_agent_present)
            .finish()
    }
}

impl fmt::Debug for ApplicationRequestContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationRequestContext")
            .field("target", &"<redacted>")
            .field("request_id", &"<redacted>")
            .field("deadline", &"<redacted>")
            .field("route", &self.route)
            .field("plane", &self.plane)
            .field("required_credential_scope", &self.required_credential_scope)
            .field(
                "trace_context",
                &self.trace_context.as_ref().map(|_| "<redacted>"),
            )
            .field("correlation", &"<redacted>")
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl fmt::Debug for ApplicationAuthenticatedRequestContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationAuthenticatedRequestContext")
            .field("request", &self.request)
            .field("principal", &self.principal.as_ref().map(|_| "<redacted>"))
            .field("assurance", &self.assurance)
            .finish()
    }
}

impl fmt::Debug for ApplicationAuthorizedRequestContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationAuthorizedRequestContext")
            .field("request", &self.authenticated.request)
            .field(
                "principal",
                &self.authenticated.principal.as_ref().map(|_| "<redacted>"),
            )
            .field("tenant", &self.tenant.map(|_| "<redacted>"))
            .field(
                "control_plane_action",
                &self.control_plane_action.as_ref().map(|_| "<redacted>"),
            )
            .field("correlation", &"<redacted>")
            .finish()
    }
}
