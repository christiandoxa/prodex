use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RequestContextPolicyPlan {
    pub required_scope: Option<CredentialScope>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RequestAuthorizationDecision {
    AllowAnonymous,
    DataPlane(BoundaryKind),
    ControlPlaneAction,
    WrongPlane,
    AnonymousNotAllowed,
    PrincipalMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RequestAuthorizationKind {
    DataPlane,
    ControlPlane,
}

pub(super) fn plan_request_context_policy(
    route: GatewayHttpRouteKind,
    plane: GatewayHttpRoutePlane,
) -> RequestContextPolicyPlan {
    #[cfg(feature = "mojo")]
    {
        use prodex_mojo_core::rich::ApplicationCredentialScopePlan;

        let plan = prodex_mojo_core::rich::plan_application_request_context(
            route_kind_tag(route),
            route_plane_tag(plane),
        )
        .expect("Mojo application request-context planner returned invalid output");
        RequestContextPolicyPlan {
            required_scope: plan.credential_scope.map(|scope| match scope {
                ApplicationCredentialScopePlan::DataPlane => CredentialScope::DataPlane,
                ApplicationCredentialScopePlan::ControlPlane => CredentialScope::ControlPlane,
            }),
        }
    }

    #[cfg(not(feature = "mojo"))]
    plan_request_context_policy_rust(route, plane)
}

pub(super) fn plan_request_authorization(
    kind: RequestAuthorizationKind,
    route: GatewayHttpRouteKind,
    plane: GatewayHttpRoutePlane,
    principal_present: bool,
    principal_matches_action: bool,
) -> RequestAuthorizationDecision {
    #[cfg(feature = "mojo")]
    {
        use prodex_mojo_core::rich::ApplicationAuthorizationDecision as Decision;

        let kind = match kind {
            RequestAuthorizationKind::DataPlane => {
                prodex_mojo_core::rich::ApplicationAuthorizationKind::DataPlane
            }
            RequestAuthorizationKind::ControlPlane => {
                prodex_mojo_core::rich::ApplicationAuthorizationKind::ControlPlane
            }
        };
        match prodex_mojo_core::rich::plan_application_authorization(
            kind,
            route_kind_tag(route),
            route_plane_tag(plane),
            principal_present,
            principal_matches_action,
        )
        .expect("Mojo application authorization planner returned invalid output")
        {
            Decision::AllowAnonymous => RequestAuthorizationDecision::AllowAnonymous,
            Decision::DataPlaneInference => {
                RequestAuthorizationDecision::DataPlane(BoundaryKind::DataPlaneInference)
            }
            Decision::DataPlaneQuota => {
                RequestAuthorizationDecision::DataPlane(BoundaryKind::DataPlaneQuota)
            }
            Decision::ControlPlaneAction => RequestAuthorizationDecision::ControlPlaneAction,
            Decision::WrongPlane => RequestAuthorizationDecision::WrongPlane,
            Decision::AnonymousNotAllowed => RequestAuthorizationDecision::AnonymousNotAllowed,
            Decision::PrincipalMismatch => RequestAuthorizationDecision::PrincipalMismatch,
        }
    }

    #[cfg(not(feature = "mojo"))]
    plan_request_authorization_rust(
        kind,
        route,
        plane,
        principal_present,
        principal_matches_action,
    )
}

#[cfg(feature = "mojo")]
fn route_plane_tag(plane: GatewayHttpRoutePlane) -> i64 {
    match plane {
        GatewayHttpRoutePlane::DataPlane => 0,
        GatewayHttpRoutePlane::ControlPlane => 1,
        GatewayHttpRoutePlane::Health => 2,
    }
}

#[cfg(feature = "mojo")]
fn route_kind_tag(route: GatewayHttpRouteKind) -> i64 {
    use GatewayHttpRouteKind::*;

    match route {
        DataPlaneResponses => 0,
        DataPlaneCompact => 1,
        DataPlaneWebSocket => 2,
        DataPlaneQuota => 3,
        DataPlaneChatCompletions => 4,
        DataPlaneEmbeddings => 5,
        DataPlaneImagesGenerations => 6,
        DataPlaneImagesEdits => 7,
        DataPlaneImagesVariations => 8,
        DataPlaneAudioSpeech => 9,
        DataPlaneAudioTranscriptions => 10,
        DataPlaneAudioTranslations => 11,
        DataPlaneBatches => 12,
        DataPlaneBatch => 13,
        DataPlaneRerank => 14,
        DataPlaneA2a => 15,
        DataPlaneMessages => 16,
        DataPlaneModels => 17,
        DataPlaneModel => 18,
        ControlPlane => 19,
        HealthLive => 20,
        HealthReady => 21,
        HealthStartup => 22,
        Unknown => 23,
    }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn plan_request_context_policy_rust(
    _route: GatewayHttpRouteKind,
    plane: GatewayHttpRoutePlane,
) -> RequestContextPolicyPlan {
    let required_scope = match plane {
        GatewayHttpRoutePlane::DataPlane => Some(CredentialScope::DataPlane),
        GatewayHttpRoutePlane::ControlPlane => Some(CredentialScope::ControlPlane),
        GatewayHttpRoutePlane::Health => None,
    };
    RequestContextPolicyPlan { required_scope }
}

#[cfg(any(not(feature = "mojo"), test))]
pub(super) fn plan_request_authorization_rust(
    kind: RequestAuthorizationKind,
    route: GatewayHttpRouteKind,
    plane: GatewayHttpRoutePlane,
    principal_present: bool,
    principal_matches_action: bool,
) -> RequestAuthorizationDecision {
    match kind {
        RequestAuthorizationKind::DataPlane if plane != GatewayHttpRoutePlane::DataPlane => {
            RequestAuthorizationDecision::WrongPlane
        }
        RequestAuthorizationKind::DataPlane if !principal_present => {
            RequestAuthorizationDecision::AllowAnonymous
        }
        RequestAuthorizationKind::DataPlane => RequestAuthorizationDecision::DataPlane(
            if route == GatewayHttpRouteKind::DataPlaneQuota {
                BoundaryKind::DataPlaneQuota
            } else {
                BoundaryKind::DataPlaneInference
            },
        ),
        RequestAuthorizationKind::ControlPlane if plane != GatewayHttpRoutePlane::ControlPlane => {
            RequestAuthorizationDecision::WrongPlane
        }
        RequestAuthorizationKind::ControlPlane if !principal_present => {
            RequestAuthorizationDecision::AnonymousNotAllowed
        }
        RequestAuthorizationKind::ControlPlane if !principal_matches_action => {
            RequestAuthorizationDecision::PrincipalMismatch
        }
        RequestAuthorizationKind::ControlPlane => RequestAuthorizationDecision::ControlPlaneAction,
    }
}
