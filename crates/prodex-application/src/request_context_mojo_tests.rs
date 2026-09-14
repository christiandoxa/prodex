use super::*;

#[test]
fn mojo_metadata_normalization_matches_rust_presence_oracle() {
    let mut headers = vec![
        GatewayHttpHeader::new(" TraceParent ", "ignored"),
        GatewayHttpHeader::new("AUTHORIZATION", "ignored"),
        GatewayHttpHeader::new("x-codex-turn-state", "ignored"),
        GatewayHttpHeader::new(" X-CODEX-BETA-FEATURES ", "ignored"),
        GatewayHttpHeader::new("User-Agent", "ignored"),
        GatewayHttpHeader::new("x-private", "ignored"),
    ];
    headers.extend(
        (headers.len()..70)
            .map(|index| GatewayHttpHeader::new(format!("x-padding-{index}"), "ignored")),
    );

    assert_eq!(
        ApplicationRequestMetadata::from_headers(&headers),
        ApplicationRequestMetadata::from_headers_rust(&headers),
    );
}

#[test]
fn mojo_request_context_policy_matches_rust_oracle_for_every_route() {
    use GatewayHttpRouteKind::*;

    let routes = [
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
    ];
    for route in routes {
        let plane = route.plane().unwrap();
        assert_eq!(
            policy::plan_request_context_policy(route, plane),
            policy::plan_request_context_policy_rust(route, plane),
        );
    }
}

#[test]
fn mojo_request_authorization_matches_rust_oracle() {
    use policy::RequestAuthorizationKind::{ControlPlane, DataPlane};

    let cases = [
        (
            DataPlane,
            GatewayHttpRouteKind::DataPlaneResponses,
            GatewayHttpRoutePlane::DataPlane,
            false,
            false,
        ),
        (
            DataPlane,
            GatewayHttpRouteKind::DataPlaneQuota,
            GatewayHttpRoutePlane::DataPlane,
            true,
            false,
        ),
        (
            DataPlane,
            GatewayHttpRouteKind::HealthReady,
            GatewayHttpRoutePlane::Health,
            false,
            false,
        ),
        (
            ControlPlane,
            GatewayHttpRouteKind::ControlPlane,
            GatewayHttpRoutePlane::ControlPlane,
            false,
            false,
        ),
        (
            ControlPlane,
            GatewayHttpRouteKind::ControlPlane,
            GatewayHttpRoutePlane::ControlPlane,
            true,
            false,
        ),
        (
            ControlPlane,
            GatewayHttpRouteKind::ControlPlane,
            GatewayHttpRoutePlane::ControlPlane,
            true,
            true,
        ),
    ];
    for (kind, route, plane, present, matches) in cases {
        assert_eq!(
            policy::plan_request_authorization(kind, route, plane, present, matches),
            policy::plan_request_authorization_rust(kind, route, plane, present, matches),
        );
    }
}
