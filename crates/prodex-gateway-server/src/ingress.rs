use std::net::{IpAddr, SocketAddr};

use bytes::Bytes;
use http_body_util::{BodyExt as _, Full};
use hyper::{
    Request, Response, StatusCode,
    body::Incoming,
    header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue},
};
use prodex_gateway_http::{
    CanonicalRequestTarget, GatewayAdminRoute, GatewayEdgeSecurityError, GatewayEdgeSecurityPolicy,
    GatewayHttpHeader, GatewayHttpRouteKind, GatewayHttpRoutePlane, classify_request_target,
    parse_gateway_admin_route, validate_gateway_edge_security,
};

use super::{GatewayBoxError, GatewayResponseBody, GatewayServerEdgeSecurity, GatewayServerMode};

pub(super) const ROUTE_UNAVAILABLE: &[u8] =
    br#"{"error":{"code":"route_not_available","message":"route is not available"}}"#;
pub(super) const INVALID_REQUEST: &[u8] =
    br#"{"error":{"code":"invalid_request","message":"request is invalid"}}"#;
pub(super) const INVALID_REQUEST_TARGET: &[u8] =
    br#"{"error":{"code":"invalid_request_target","message":"request target is invalid"}}"#;
pub(super) const BODY_TOO_LARGE: &[u8] = br#"{"error":{"code":"request_body_too_large","message":"request body exceeds the configured limit"}}"#;
pub(super) const BACKEND_TIMEOUT: &[u8] =
    br#"{"error":{"code":"backend_timeout","message":"gateway backend timed out"}}"#;
pub(super) const SERVICE_UNAVAILABLE: &[u8] =
    br#"{"error":{"code":"service_unavailable","message":"gateway backend is unavailable"}}"#;
pub(super) const LOCAL_OVERLOAD: &[u8] =
    br#"{"error":{"code":"service_unavailable","message":"gateway is temporarily overloaded"}}"#;
pub(super) const EDGE_REQUEST_DENIED: &[u8] =
    br#"{"error":{"code":"edge_request_denied","message":"gateway edge request is denied"}}"#;
const MAX_FORWARDED_FOR_HOPS: usize = 16;

type ParsedIngressRequest = (
    CanonicalRequestTarget,
    GatewayHttpRouteKind,
    GatewayHttpRoutePlane,
    Vec<GatewayHttpHeader>,
);

pub(super) fn parse_ingress_request(
    request: &Request<Incoming>,
    mode: GatewayServerMode,
) -> Result<ParsedIngressRequest, Box<Response<GatewayResponseBody>>> {
    if request.uri().scheme().is_some() || request.uri().authority().is_some() {
        return Err(Box::new(json_error(
            StatusCode::BAD_REQUEST,
            INVALID_REQUEST_TARGET,
        )));
    }
    let raw_target = request
        .uri()
        .path_and_query()
        .map_or_else(|| request.uri().path(), |target| target.as_str());
    let target = CanonicalRequestTarget::parse(raw_target)
        .map_err(|_| Box::new(json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST_TARGET)))?;
    let route = classify_request_target(&target)
        .ok_or_else(|| Box::new(json_error(StatusCode::NOT_FOUND, ROUTE_UNAVAILABLE)))?;
    if !route_allowed(mode, &target, route.plane) {
        return Err(Box::new(json_error(
            StatusCode::NOT_FOUND,
            ROUTE_UNAVAILABLE,
        )));
    }
    let (kind, plane) = (route.kind, route.plane);
    let headers = gateway_http_headers(request)
        .ok_or_else(|| Box::new(json_error(StatusCode::BAD_REQUEST, INVALID_REQUEST)))?;
    Ok((target, kind, plane, headers))
}

pub(super) fn validate_ingress_security(
    request: &Request<Incoming>,
    plane: GatewayHttpRoutePlane,
    peer_addr: SocketAddr,
    edge_security: &GatewayServerEdgeSecurity,
    headers: &[GatewayHttpHeader],
) -> Result<(IpAddr, bool), Box<Response<GatewayResponseBody>>> {
    let peer_is_trusted_proxy = edge_security.trusted_proxies.contains(&peer_addr.ip());
    let client_ip = derive_gateway_client_ip(peer_addr, &edge_security.trusted_proxies, headers)
        .map_err(|_| Box::new(json_error(StatusCode::FORBIDDEN, EDGE_REQUEST_DENIED)))?;
    if plane == GatewayHttpRoutePlane::ControlPlane {
        validate_control_plane_security(request, peer_is_trusted_proxy, edge_security, headers)?;
    }
    if plane == GatewayHttpRoutePlane::DataPlane
        && (request.headers().contains_key(hyper::header::ORIGIN)
            || request.headers().contains_key(hyper::header::COOKIE))
    {
        return Err(Box::new(json_error(
            StatusCode::FORBIDDEN,
            EDGE_REQUEST_DENIED,
        )));
    }
    Ok((client_ip, peer_is_trusted_proxy))
}

fn validate_control_plane_security(
    request: &Request<Incoming>,
    peer_is_trusted_proxy: bool,
    edge_security: &GatewayServerEdgeSecurity,
    headers: &[GatewayHttpHeader],
) -> Result<(), Box<Response<GatewayResponseBody>>> {
    let browser = if browser_capable_request(request) {
        edge_security
            .browser
            .as_ref()
            .ok_or_else(|| Box::new(json_error(StatusCode::FORBIDDEN, EDGE_REQUEST_DENIED)))?
    } else {
        return validate_gateway_edge_security(
            GatewayEdgeSecurityPolicy {
                peer_is_trusted_proxy,
                expected_host: loopback_compatible_expected_host(
                    &edge_security.expected_host,
                    headers,
                ),
                expected_origin: None,
                expected_csrf_token: None,
            },
            headers,
        )
        .map_err(|_| Box::new(json_error(StatusCode::FORBIDDEN, EDGE_REQUEST_DENIED)));
    };
    let state_changing = state_changing_method(request.method());
    let expected_origin = (state_changing || request.headers().contains_key(hyper::header::ORIGIN))
        .then_some(browser.expected_origin.as_str());
    let expected_csrf_token = state_changing
        .then_some(browser.expected_csrf_token.as_deref())
        .flatten();
    validate_gateway_edge_security(
        GatewayEdgeSecurityPolicy {
            peer_is_trusted_proxy,
            expected_host: loopback_compatible_expected_host(&edge_security.expected_host, headers),
            expected_origin,
            expected_csrf_token,
        },
        headers,
    )
    .map_err(|_| Box::new(json_error(StatusCode::FORBIDDEN, EDGE_REQUEST_DENIED)))
}

pub(super) fn validate_request_size(
    request: &Request<Incoming>,
    max_request_body_bytes: usize,
) -> Result<(), Box<Response<GatewayResponseBody>>> {
    match content_length(request) {
        Err(()) => Err(Box::new(json_error(
            StatusCode::BAD_REQUEST,
            INVALID_REQUEST,
        ))),
        Ok(Some(length)) if length > max_request_body_bytes as u64 => Err(Box::new(json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            BODY_TOO_LARGE,
        ))),
        Ok(_) => Ok(()),
    }
}

pub(super) fn strip_forwarding_headers(request: &mut Request<Incoming>) {
    let forwarding_headers = request
        .headers()
        .keys()
        .filter(|name| {
            name.as_str() == "forwarded"
                || name.as_str().starts_with("x-forwarded-")
                || name.as_str() == "x-real-ip"
        })
        .cloned()
        .collect::<Vec<_>>();
    for name in forwarding_headers {
        request.headers_mut().remove(name);
    }
}

fn gateway_http_headers(request: &Request<Incoming>) -> Option<Vec<GatewayHttpHeader>> {
    request
        .headers()
        .iter()
        .map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| GatewayHttpHeader::new(name.as_str(), value))
        })
        .collect()
}

/// Derives client network metadata only from the authenticated transport peer.
/// The right-most untrusted address defeats caller-prepended spoofed hops.
fn derive_gateway_client_ip(
    peer_addr: SocketAddr,
    trusted_proxies: &[IpAddr],
    headers: &[GatewayHttpHeader],
) -> Result<IpAddr, GatewayEdgeSecurityError> {
    let peer_is_trusted_proxy = trusted_proxies.contains(&peer_addr.ip());
    let has_forwarding_metadata = headers.iter().any(|header| {
        matches!(
            header.normalized_name().as_str(),
            "forwarded"
                | "x-forwarded-for"
                | "x-forwarded-host"
                | "x-forwarded-proto"
                | "x-real-ip"
        )
    });
    if has_forwarding_metadata && !peer_is_trusted_proxy {
        return Err(GatewayEdgeSecurityError::ForwardedHeaderFromUntrustedPeer);
    }
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == "x-forwarded-for")
        .map(|header| header.value.as_str());
    let Some(value) = values.next() else {
        return Ok(peer_addr.ip());
    };
    if values.next().is_some() {
        return Err(GatewayEdgeSecurityError::ForwardedClientAddressInvalid);
    }
    let hops = value
        .split(',')
        .map(str::trim)
        .map(str::parse::<IpAddr>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| GatewayEdgeSecurityError::ForwardedClientAddressInvalid)?;
    if hops.is_empty() || hops.len() > MAX_FORWARDED_FOR_HOPS {
        return Err(GatewayEdgeSecurityError::ForwardedClientAddressInvalid);
    }
    Ok(hops
        .iter()
        .rev()
        .find(|address| !trusted_proxies.contains(address))
        .copied()
        .unwrap_or(hops[0]))
}

fn loopback_compatible_expected_host<'a>(
    configured: &'a str,
    headers: &'a [GatewayHttpHeader],
) -> &'a str {
    let Ok(expected) = configured.parse::<SocketAddr>() else {
        return configured;
    };
    if !expected.ip().is_loopback() {
        return configured;
    }
    let mut hosts = headers
        .iter()
        .filter(|header| header.normalized_name() == "host")
        .map(|header| header.value.as_str());
    let Some(host) = hosts.next() else {
        return configured;
    };
    if hosts.next().is_some() {
        return configured;
    }
    let Ok(authority) = host.parse::<hyper::http::uri::Authority>() else {
        return configured;
    };
    if authority.port_u16().unwrap_or(80) != expected.port() {
        return configured;
    }
    let name = authority.host().trim_matches(['[', ']']);
    if name.eq_ignore_ascii_case("localhost")
        || name
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
    {
        host
    } else {
        configured
    }
}

fn browser_capable_request(request: &Request<Incoming>) -> bool {
    [
        "origin",
        "cookie",
        "sec-fetch-site",
        "sec-fetch-mode",
        "sec-fetch-dest",
        "x-csrf-token",
    ]
    .into_iter()
    .any(|name| request.headers().contains_key(name))
}

fn state_changing_method(method: &hyper::Method) -> bool {
    !matches!(
        *method,
        hyper::Method::GET | hyper::Method::HEAD | hyper::Method::OPTIONS
    )
}

pub(super) fn route_allowed(
    mode: GatewayServerMode,
    target: &CanonicalRequestTarget,
    plane: GatewayHttpRoutePlane,
) -> bool {
    matches!(plane, GatewayHttpRoutePlane::Health)
        || matches!(
            (mode, plane),
            (
                GatewayServerMode::DataPlane,
                GatewayHttpRoutePlane::DataPlane
            ) | (
                GatewayServerMode::ControlPlane,
                GatewayHttpRoutePlane::ControlPlane
            )
        )
        || (mode == GatewayServerMode::DataPlane
            && plane == GatewayHttpRoutePlane::ControlPlane
            && matches!(
                gateway_admin_route(target),
                Some(GatewayAdminRoute::Metrics)
            ))
}

fn gateway_admin_route(target: &CanonicalRequestTarget) -> Option<GatewayAdminRoute<'_>> {
    parse_gateway_admin_route("", target.path())
        .or_else(|| parse_gateway_admin_route("/v1", target.path()))
}

fn content_length(request: &Request<Incoming>) -> Result<Option<u64>, ()> {
    request
        .headers()
        .get(CONTENT_LENGTH)
        .map(|value| value.to_str().map_err(|_| ())?.parse().map_err(|_| ()))
        .transpose()
}

pub(super) fn json_error(status: StatusCode, body: &'static [u8]) -> Response<GatewayResponseBody> {
    let mut response = Response::new(
        Full::new(Bytes::from_static(body))
            .map_err(|error: std::convert::Infallible| -> GatewayBoxError { match error {} })
            .boxed_unsync(),
    );
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
