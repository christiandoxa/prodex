use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpRouteKind {
    DataPlaneResponses,
    DataPlaneCompact,
    DataPlaneWebSocket,
    DataPlaneQuota,
    DataPlaneChatCompletions,
    DataPlaneEmbeddings,
    DataPlaneImagesGenerations,
    DataPlaneImagesEdits,
    DataPlaneImagesVariations,
    DataPlaneAudioSpeech,
    DataPlaneAudioTranscriptions,
    DataPlaneAudioTranslations,
    DataPlaneBatches,
    DataPlaneBatch,
    DataPlaneRerank,
    DataPlaneA2a,
    DataPlaneMessages,
    DataPlaneModels,
    DataPlaneModel,
    ControlPlane,
    HealthLive,
    HealthReady,
    HealthStartup,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpRoutePlane {
    DataPlane,
    ControlPlane,
    Health,
}

impl GatewayHttpRouteKind {
    pub const fn plane(self) -> Option<GatewayHttpRoutePlane> {
        match self {
            Self::DataPlaneResponses
            | Self::DataPlaneCompact
            | Self::DataPlaneWebSocket
            | Self::DataPlaneQuota
            | Self::DataPlaneChatCompletions
            | Self::DataPlaneEmbeddings
            | Self::DataPlaneImagesGenerations
            | Self::DataPlaneImagesEdits
            | Self::DataPlaneImagesVariations
            | Self::DataPlaneAudioSpeech
            | Self::DataPlaneAudioTranscriptions
            | Self::DataPlaneAudioTranslations
            | Self::DataPlaneBatches
            | Self::DataPlaneBatch
            | Self::DataPlaneRerank
            | Self::DataPlaneA2a
            | Self::DataPlaneMessages
            | Self::DataPlaneModels
            | Self::DataPlaneModel => Some(GatewayHttpRoutePlane::DataPlane),
            Self::ControlPlane => Some(GatewayHttpRoutePlane::ControlPlane),
            Self::HealthLive | Self::HealthReady | Self::HealthStartup => {
                Some(GatewayHttpRoutePlane::Health)
            }
            Self::Unknown => None,
        }
    }

    pub const fn is_data_plane(self) -> bool {
        matches!(self.plane(), Some(GatewayHttpRoutePlane::DataPlane))
    }

    pub const fn is_provider_data_plane(self) -> bool {
        self.is_data_plane() && !matches!(self, Self::DataPlaneQuota)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayHttpRoute<'a> {
    pub target: &'a CanonicalRequestTarget,
    pub kind: GatewayHttpRouteKind,
    pub plane: GatewayHttpRoutePlane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayControlPlaneOperation {
    GatewayAdminRead,
    RouteExplain,
    TenantCreate,
    TenantUpdate,
    UserInvite,
    ScimUserRead,
    ScimUserCreate,
    ScimUserUpdate,
    ScimUserDelete,
    RoleBindingGrant,
    RoleBindingRevoke,
    ServiceIdentityCreate,
    VirtualKeyRead,
    VirtualKeyCreate,
    VirtualKeyUpdate,
    VirtualKeyDelete,
    VirtualKeyRotateSecret,
    PolicyPublish,
    ProviderCredentialRotate,
    BudgetUpdate,
    BillingRead,
    AuditExport,
    AuditRetentionPurge,
    ConfigurationPublish,
}

impl GatewayControlPlaneOperation {
    pub const fn requires_idempotency(self) -> bool {
        !matches!(
            self,
            Self::GatewayAdminRead
                | Self::RouteExplain
                | Self::ScimUserRead
                | Self::VirtualKeyRead
                | Self::BillingRead
                | Self::AuditExport
        )
    }

    pub const fn requires_audit(self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayControlPlaneRoutePlan {
    pub operation: GatewayControlPlaneOperation,
    pub requires_idempotency: bool,
    pub requires_audit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayControlPlaneRouteError {
    NotControlPlaneRoute,
    UnknownControlPlaneRoute,
    MethodNotAllowed {
        operation: GatewayControlPlaneOperation,
        method: GatewayHttpMethod,
    },
}

impl fmt::Display for GatewayControlPlaneRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotControlPlaneRoute | Self::UnknownControlPlaneRoute => {
                write!(f, "HTTP route is invalid")
            }
            Self::MethodNotAllowed { .. } => write!(f, "HTTP method is not allowed"),
        }
    }
}

impl Error for GatewayControlPlaneRouteError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayControlPlaneRouteErrorStatus {
    BadRequest,
    MethodNotAllowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayControlPlaneRouteErrorResponsePlan {
    pub status: GatewayControlPlaneRouteErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_control_plane_route_error_response(
    error: &GatewayControlPlaneRouteError,
) -> GatewayControlPlaneRouteErrorResponsePlan {
    match error {
        GatewayControlPlaneRouteError::NotControlPlaneRoute
        | GatewayControlPlaneRouteError::UnknownControlPlaneRoute => {
            GatewayControlPlaneRouteErrorResponsePlan {
                status: GatewayControlPlaneRouteErrorStatus::BadRequest,
                code: "control_plane_route_invalid",
                message: "control-plane route is invalid",
            }
        }
        GatewayControlPlaneRouteError::MethodNotAllowed { .. } => {
            GatewayControlPlaneRouteErrorResponsePlan {
                status: GatewayControlPlaneRouteErrorStatus::MethodNotAllowed,
                code: "control_plane_method_not_allowed",
                message: "HTTP method is not allowed for this control-plane route",
            }
        }
    }
}

pub fn classify_route(path: &str) -> GatewayHttpRouteKind {
    CanonicalRequestTarget::parse(path)
        .as_ref()
        .map_or(GatewayHttpRouteKind::Unknown, |target| {
            classify_canonical_route(target)
        })
}

pub fn classify_request_target(target: &CanonicalRequestTarget) -> Option<GatewayHttpRoute<'_>> {
    let kind = classify_canonical_route(target);
    Some(GatewayHttpRoute {
        target,
        kind,
        plane: kind.plane()?,
    })
}

pub fn classify_canonical_route(target: &CanonicalRequestTarget) -> GatewayHttpRouteKind {
    if parsed_gateway_admin_route(target.path()).is_some() {
        return GatewayHttpRouteKind::ControlPlane;
    }
    let path = canonical_control_plane_path(target.path());
    match path.as_ref() {
        "/livez" => GatewayHttpRouteKind::HealthLive,
        "/readyz" => GatewayHttpRouteKind::HealthReady,
        "/startupz" => GatewayHttpRouteKind::HealthStartup,
        "/v1/responses" | "/responses" => GatewayHttpRouteKind::DataPlaneResponses,
        "/v1/responses/compact" | "/responses/compact" => GatewayHttpRouteKind::DataPlaneCompact,
        "/v1/realtime" | "/realtime" => GatewayHttpRouteKind::DataPlaneWebSocket,
        "/v1/quota" | "/quota" => GatewayHttpRouteKind::DataPlaneQuota,
        "/v1/chat/completions" | "/chat/completions" => {
            GatewayHttpRouteKind::DataPlaneChatCompletions
        }
        "/v1/embeddings" | "/embeddings" => GatewayHttpRouteKind::DataPlaneEmbeddings,
        "/v1/images/generations" | "/images/generations" => {
            GatewayHttpRouteKind::DataPlaneImagesGenerations
        }
        "/v1/images/edits" | "/images/edits" => GatewayHttpRouteKind::DataPlaneImagesEdits,
        "/v1/images/variations" | "/images/variations" => {
            GatewayHttpRouteKind::DataPlaneImagesVariations
        }
        "/v1/audio/speech" | "/audio/speech" => GatewayHttpRouteKind::DataPlaneAudioSpeech,
        "/v1/audio/transcriptions" | "/audio/transcriptions" => {
            GatewayHttpRouteKind::DataPlaneAudioTranscriptions
        }
        "/v1/audio/translations" | "/audio/translations" => {
            GatewayHttpRouteKind::DataPlaneAudioTranslations
        }
        "/v1/batches" | "/batches" => GatewayHttpRouteKind::DataPlaneBatches,
        path if has_nonempty_suffix(path, "/v1/batches/")
            || has_nonempty_suffix(path, "/batches/") =>
        {
            GatewayHttpRouteKind::DataPlaneBatch
        }
        "/v1/rerank" | "/rerank" => GatewayHttpRouteKind::DataPlaneRerank,
        "/v1/a2a" | "/a2a" => GatewayHttpRouteKind::DataPlaneA2a,
        "/v1/messages" | "/messages" => GatewayHttpRouteKind::DataPlaneMessages,
        "/v1/models" | "/models" => GatewayHttpRouteKind::DataPlaneModels,
        path if has_nonempty_suffix(path, "/v1/models/")
            || has_nonempty_suffix(path, "/models/") =>
        {
            GatewayHttpRouteKind::DataPlaneModel
        }
        path if is_known_control_plane_route(path) => GatewayHttpRouteKind::ControlPlane,
        _ => GatewayHttpRouteKind::Unknown,
    }
}

fn has_nonempty_suffix(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix)
        .is_some_and(|suffix| !suffix.is_empty())
}

pub fn plan_control_plane_route(
    request: &GatewayHttpRequestMeta,
) -> Result<GatewayControlPlaneRoutePlan, GatewayControlPlaneRouteError> {
    let target = CanonicalRequestTarget::parse(request.path.as_str())
        .map_err(|_| GatewayControlPlaneRouteError::NotControlPlaneRoute)?;
    if !is_control_plane_mount(target.path()) {
        return Err(GatewayControlPlaneRouteError::NotControlPlaneRoute);
    }
    let operation = control_plane_operation_for_path(target.path(), request.method)
        .ok_or(GatewayControlPlaneRouteError::UnknownControlPlaneRoute)?;
    if !control_plane_operation_allows_method(operation, request.method) {
        return Err(GatewayControlPlaneRouteError::MethodNotAllowed {
            operation,
            method: request.method,
        });
    }
    Ok(GatewayControlPlaneRoutePlan {
        operation,
        requires_idempotency: operation.requires_idempotency()
            && !matches!(
                request.method,
                GatewayHttpMethod::Get | GatewayHttpMethod::Options
            ),
        requires_audit: operation.requires_audit(),
    })
}

fn is_control_plane_mount(path: &str) -> bool {
    let path = canonical_control_plane_path(path);
    ["/admin", "/v1/admin", "/scim", "/v1/scim"]
        .into_iter()
        .any(|mount| {
            path == mount
                || path
                    .strip_prefix(mount)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
}

fn is_known_control_plane_route(path: &str) -> bool {
    [
        GatewayHttpMethod::Get,
        GatewayHttpMethod::Post,
        GatewayHttpMethod::Put,
        GatewayHttpMethod::Patch,
        GatewayHttpMethod::Delete,
        GatewayHttpMethod::Options,
    ]
    .into_iter()
    .any(|method| control_plane_operation_for_path(path, method).is_some())
}

fn control_plane_operation_for_path(
    path: &str,
    method: GatewayHttpMethod,
) -> Option<GatewayControlPlaneOperation> {
    if let Some(route) = parsed_gateway_admin_route(path) {
        return route.operation(method);
    }
    let path = canonical_control_plane_path(path);
    let path = path.as_ref();
    if let Some(scim_path) = strip_control_plane_mount(path, "/scim")
        .or_else(|| strip_control_plane_mount(path, "/v1/scim"))
    {
        return match normalized_segments(scim_path).as_slice() {
            ["v2", "Users"] if method == GatewayHttpMethod::Get => {
                Some(GatewayControlPlaneOperation::ScimUserRead)
            }
            ["v2", "Users"] => Some(GatewayControlPlaneOperation::ScimUserCreate),
            ["v2", "Users", ..] if method == GatewayHttpMethod::Get => {
                Some(GatewayControlPlaneOperation::ScimUserRead)
            }
            ["v2", "Users", ..] if method == GatewayHttpMethod::Delete => {
                Some(GatewayControlPlaneOperation::ScimUserDelete)
            }
            ["v2", "Users", ..] => Some(GatewayControlPlaneOperation::ScimUserUpdate),
            _ => None,
        };
    }
    let admin_path = strip_control_plane_mount(path, "/admin")
        .or_else(|| strip_control_plane_mount(path, "/v1/admin"))?;
    match normalized_segments(admin_path).as_slice() {
        []
        | ["openapi.json"]
        | ["metrics"]
        | ["providers"]
        | ["observability"]
        | ["guardrails"]
        | ["usage"] => Some(GatewayControlPlaneOperation::GatewayAdminRead),
        ["routes", "explain"] => Some(GatewayControlPlaneOperation::RouteExplain),
        ["tenants"] => Some(GatewayControlPlaneOperation::TenantCreate),
        ["tenants", ..] => Some(GatewayControlPlaneOperation::TenantUpdate),
        ["users"] | ["users", "invites"] => Some(GatewayControlPlaneOperation::UserInvite),
        ["role-bindings"] => Some(GatewayControlPlaneOperation::RoleBindingGrant),
        ["role-bindings", ..] => Some(GatewayControlPlaneOperation::RoleBindingRevoke),
        ["service-identities"] => Some(GatewayControlPlaneOperation::ServiceIdentityCreate),
        ["keys", _, "secret"] | ["keys", _, "secrets"] => {
            Some(GatewayControlPlaneOperation::VirtualKeyRotateSecret)
        }
        ["keys"] if method == GatewayHttpMethod::Get => {
            Some(GatewayControlPlaneOperation::VirtualKeyRead)
        }
        ["keys"] => Some(GatewayControlPlaneOperation::VirtualKeyCreate),
        ["keys", ..] if method == GatewayHttpMethod::Get => {
            Some(GatewayControlPlaneOperation::VirtualKeyRead)
        }
        ["keys", ..] if method == GatewayHttpMethod::Patch => {
            Some(GatewayControlPlaneOperation::VirtualKeyUpdate)
        }
        ["keys", ..] if method == GatewayHttpMethod::Delete => {
            Some(GatewayControlPlaneOperation::VirtualKeyDelete)
        }
        ["provider-credentials", _, "secret"] | ["provider-credentials", _, "secrets"] => {
            Some(GatewayControlPlaneOperation::ProviderCredentialRotate)
        }
        ["budgets", ..] => Some(GatewayControlPlaneOperation::BudgetUpdate),
        [
            "policies"
            | "classification-rules"
            | "provider-registries"
            | "routing-scores"
            | "execution-approvals",
            ..,
        ] => Some(GatewayControlPlaneOperation::PolicyPublish),
        ["sessions", _, "revoke"] => Some(GatewayControlPlaneOperation::PolicyPublish),
        ["governance", "outbox", ..] => Some(GatewayControlPlaneOperation::PolicyPublish),
        ["governance", "audit", "integrity"] => Some(GatewayControlPlaneOperation::PolicyPublish),
        ["configuration", ..] | ["config", ..] => {
            Some(GatewayControlPlaneOperation::ConfigurationPublish)
        }
        ["billing", ..] => Some(GatewayControlPlaneOperation::BillingRead),
        ["ledger"] | ["ledger.csv"] | ["ledger", ..] => {
            Some(GatewayControlPlaneOperation::BillingRead)
        }
        ["audit", "exports"] | ["audit", "exports", ..] => {
            Some(GatewayControlPlaneOperation::AuditExport)
        }
        ["audit", "retention"] | ["audit", "retention", ..] => {
            Some(GatewayControlPlaneOperation::AuditRetentionPurge)
        }
        _ => None,
    }
}

fn parsed_gateway_admin_route(path: &str) -> Option<GatewayAdminRoute<'_>> {
    parse_gateway_admin_route("", path).or_else(|| parse_gateway_admin_route("/v1", path))
}

fn canonical_control_plane_path(path: &str) -> Cow<'_, str> {
    for mount in ["/prodex/gateway", "/v1/prodex/gateway"] {
        if path == mount {
            return Cow::Borrowed("/admin");
        }
        let Some(suffix) = path
            .strip_prefix(mount)
            .and_then(|suffix| suffix.strip_prefix('/'))
        else {
            continue;
        };
        if suffix.starts_with("scim/") {
            return Cow::Owned(format!("/{suffix}"));
        }
        if suffix == "admin" {
            return Cow::Borrowed("/admin");
        }
        return Cow::Owned(format!("/admin/{suffix}"));
    }
    Cow::Borrowed(path)
}

pub(super) fn path_without_query_or_fragment(path: &str) -> &str {
    path.trim()
        .split_once(['?', '#'])
        .map_or_else(|| path.trim(), |(path, _)| path)
}

fn strip_control_plane_mount<'a>(path: &'a str, mount: &str) -> Option<&'a str> {
    if path == mount {
        Some("")
    } else {
        path.strip_prefix(mount)
            .and_then(|suffix| suffix.strip_prefix('/'))
    }
}

fn normalized_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn control_plane_operation_allows_method(
    operation: GatewayControlPlaneOperation,
    method: GatewayHttpMethod,
) -> bool {
    match operation {
        GatewayControlPlaneOperation::GatewayAdminRead
        | GatewayControlPlaneOperation::ScimUserRead
        | GatewayControlPlaneOperation::VirtualKeyRead
        | GatewayControlPlaneOperation::BillingRead => method == GatewayHttpMethod::Get,
        GatewayControlPlaneOperation::RouteExplain
        | GatewayControlPlaneOperation::TenantCreate
        | GatewayControlPlaneOperation::UserInvite
        | GatewayControlPlaneOperation::ScimUserCreate
        | GatewayControlPlaneOperation::RoleBindingGrant
        | GatewayControlPlaneOperation::ServiceIdentityCreate
        | GatewayControlPlaneOperation::VirtualKeyCreate
        | GatewayControlPlaneOperation::VirtualKeyRotateSecret
        | GatewayControlPlaneOperation::ProviderCredentialRotate
        | GatewayControlPlaneOperation::ConfigurationPublish
        | GatewayControlPlaneOperation::AuditExport => method == GatewayHttpMethod::Post,
        GatewayControlPlaneOperation::PolicyPublish => {
            matches!(method, GatewayHttpMethod::Get | GatewayHttpMethod::Post)
        }
        GatewayControlPlaneOperation::TenantUpdate
        | GatewayControlPlaneOperation::VirtualKeyUpdate
        | GatewayControlPlaneOperation::BudgetUpdate => method == GatewayHttpMethod::Patch,
        GatewayControlPlaneOperation::ScimUserUpdate => {
            matches!(method, GatewayHttpMethod::Patch | GatewayHttpMethod::Put)
        }
        GatewayControlPlaneOperation::ScimUserDelete
        | GatewayControlPlaneOperation::RoleBindingRevoke
        | GatewayControlPlaneOperation::VirtualKeyDelete
        | GatewayControlPlaneOperation::AuditRetentionPurge => method == GatewayHttpMethod::Delete,
    }
}

pub(super) fn validate_method(
    route: GatewayHttpRouteKind,
    method: GatewayHttpMethod,
) -> Result<(), GatewayHttpPlanError> {
    let allowed = match route {
        GatewayHttpRouteKind::HealthLive
        | GatewayHttpRouteKind::HealthReady
        | GatewayHttpRouteKind::HealthStartup => method == GatewayHttpMethod::Get,
        GatewayHttpRouteKind::DataPlaneResponses
        | GatewayHttpRouteKind::DataPlaneCompact
        | GatewayHttpRouteKind::DataPlaneChatCompletions
        | GatewayHttpRouteKind::DataPlaneEmbeddings
        | GatewayHttpRouteKind::DataPlaneImagesGenerations
        | GatewayHttpRouteKind::DataPlaneImagesEdits
        | GatewayHttpRouteKind::DataPlaneImagesVariations
        | GatewayHttpRouteKind::DataPlaneAudioSpeech
        | GatewayHttpRouteKind::DataPlaneAudioTranscriptions
        | GatewayHttpRouteKind::DataPlaneAudioTranslations
        | GatewayHttpRouteKind::DataPlaneRerank
        | GatewayHttpRouteKind::DataPlaneA2a
        | GatewayHttpRouteKind::DataPlaneMessages => method == GatewayHttpMethod::Post,
        GatewayHttpRouteKind::DataPlaneWebSocket | GatewayHttpRouteKind::DataPlaneQuota => {
            method == GatewayHttpMethod::Get
        }
        GatewayHttpRouteKind::DataPlaneBatches => {
            matches!(method, GatewayHttpMethod::Get | GatewayHttpMethod::Post)
        }
        GatewayHttpRouteKind::DataPlaneBatch => matches!(
            method,
            GatewayHttpMethod::Get | GatewayHttpMethod::Post | GatewayHttpMethod::Delete
        ),
        GatewayHttpRouteKind::DataPlaneModels | GatewayHttpRouteKind::DataPlaneModel => {
            method == GatewayHttpMethod::Get
        }
        GatewayHttpRouteKind::ControlPlane => matches!(
            method,
            GatewayHttpMethod::Get
                | GatewayHttpMethod::Post
                | GatewayHttpMethod::Put
                | GatewayHttpMethod::Patch
                | GatewayHttpMethod::Delete
                | GatewayHttpMethod::Options
        ),
        GatewayHttpRouteKind::Unknown => true,
    };
    if allowed {
        Ok(())
    } else {
        Err(GatewayHttpPlanError::MethodNotAllowed { route, method })
    }
}

pub(super) fn requires_trace(route: GatewayHttpRouteKind) -> bool {
    route.is_data_plane() || route == GatewayHttpRouteKind::ControlPlane
}

pub(super) fn requires_cancellation_propagation(route: GatewayHttpRouteKind) -> bool {
    route.is_provider_data_plane()
}

pub(super) fn requires_streaming_backpressure(route: GatewayHttpRouteKind) -> bool {
    matches!(
        route,
        GatewayHttpRouteKind::DataPlaneResponses
            | GatewayHttpRouteKind::DataPlaneWebSocket
            | GatewayHttpRouteKind::DataPlaneChatCompletions
            | GatewayHttpRouteKind::DataPlaneMessages
            | GatewayHttpRouteKind::DataPlaneAudioSpeech
    )
}

pub fn trace_context_from_headers(
    headers: &[GatewayHttpHeader],
) -> Result<Option<TraceContext>, GatewayHttpPlanError> {
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == "traceparent");
    let Some(header) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(GatewayHttpPlanError::DuplicateTraceContext);
    }
    TraceContext::parse_traceparent(&header.value)
        .map(Some)
        .map_err(GatewayHttpPlanError::InvalidTraceContext)
}

pub(super) fn trace_propagation_metrics_from_headers(
    headers: &[GatewayHttpHeader],
) -> Vec<TracePropagationMetricPlan> {
    [
        (TracePropagationCarrier::Traceparent, "traceparent"),
        (TracePropagationCarrier::Tracestate, "tracestate"),
        (TracePropagationCarrier::Baggage, "baggage"),
    ]
    .into_iter()
    .filter_map(|(carrier, name)| {
        let result = if headers
            .iter()
            .any(|header| header.normalized_name() == name)
        {
            TracePropagationResult::Propagated
        } else {
            TracePropagationResult::Missing
        };
        plan_trace_propagation_metric(carrier, result).ok()
    })
    .collect()
}

pub(super) fn validate_singleton_credential_headers(
    headers: &[GatewayHttpHeader],
) -> Result<(), GatewayHttpPlanError> {
    let authorization_count = headers
        .iter()
        .filter(|header| header.normalized_name() == "authorization")
        .take(2)
        .count();
    if authorization_count > 1 {
        return Err(GatewayHttpPlanError::DuplicateAuthorization);
    }
    let account_count = headers
        .iter()
        .filter(|header| header.normalized_name() == "chatgpt-account-id")
        .take(2)
        .count();
    if account_count > 1 {
        return Err(GatewayHttpPlanError::DuplicateChatGptAccountId);
    }

    Ok(())
}

pub(super) fn validate_singleton_affinity_headers(
    headers: &[GatewayHttpHeader],
) -> Result<(), GatewayHttpPlanError> {
    let session_count = headers
        .iter()
        .filter(|header| header.normalized_name() == "session_id")
        .take(2)
        .count();
    if session_count > 1 {
        return Err(GatewayHttpPlanError::DuplicateSessionId);
    }

    let turn_state_count = headers
        .iter()
        .filter(|header| header.normalized_name() == "x-codex-turn-state")
        .take(2)
        .count();
    if turn_state_count > 1 {
        return Err(GatewayHttpPlanError::DuplicateCodexTurnState);
    }

    Ok(())
}

pub(super) fn validate_singleton_codex_metadata_headers(
    headers: &[GatewayHttpHeader],
) -> Result<(), GatewayHttpPlanError> {
    for name in [
        "x-openai-subagent",
        "x-codex-turn-metadata",
        "x-codex-beta-features",
    ] {
        let count = headers
            .iter()
            .filter(|header| header.normalized_name() == name)
            .take(2)
            .count();
        if count > 1 {
            return Err(GatewayHttpPlanError::DuplicateCodexMetadata);
        }
    }
    Ok(())
}
