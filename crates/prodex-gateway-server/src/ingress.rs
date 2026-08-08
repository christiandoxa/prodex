use super::*;

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
