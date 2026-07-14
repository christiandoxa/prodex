use crate::GatewayHttpHeader;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy)]
pub struct GatewayEdgeSecurityPolicy<'a> {
    /// Trusted transport evidence supplied by the network adapter.
    pub peer_is_trusted_proxy: bool,
    pub expected_host: &'a str,
    pub expected_origin: Option<&'a str>,
    pub expected_csrf_token: Option<&'a str>,
}

impl fmt::Debug for GatewayEdgeSecurityPolicy<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayEdgeSecurityPolicy")
            .field("peer_is_trusted_proxy", &self.peer_is_trusted_proxy)
            .field("expected_host", &"<redacted>")
            .field(
                "expected_origin",
                &self.expected_origin.map(|_| "<redacted>"),
            )
            .field(
                "expected_csrf_token",
                &self.expected_csrf_token.map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayEdgeSecurityError {
    ForwardedHeaderFromUntrustedPeer,
    HostMissingOrDuplicated,
    HostMismatch,
    OriginMissingOrDuplicated,
    OriginMismatch,
    CsrfMissingOrDuplicated,
    CsrfMismatch,
    ForwardedClientAddressInvalid,
}

impl fmt::Display for GatewayEdgeSecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("gateway edge request is denied")
    }
}

impl Error for GatewayEdgeSecurityError {}

pub fn validate_gateway_edge_security(
    policy: GatewayEdgeSecurityPolicy<'_>,
    headers: &[GatewayHttpHeader],
) -> Result<(), GatewayEdgeSecurityError> {
    let forwarded = headers.iter().any(is_forwarded_header);
    if forwarded && !policy.peer_is_trusted_proxy {
        return Err(GatewayEdgeSecurityError::ForwardedHeaderFromUntrustedPeer);
    }
    let host =
        unique_header(headers, "host").ok_or(GatewayEdgeSecurityError::HostMissingOrDuplicated)?;
    if !host.eq_ignore_ascii_case(policy.expected_host) {
        return Err(GatewayEdgeSecurityError::HostMismatch);
    }
    if let Some(expected_origin) = policy.expected_origin {
        let origin = unique_header(headers, "origin")
            .ok_or(GatewayEdgeSecurityError::OriginMissingOrDuplicated)?;
        if origin != expected_origin {
            return Err(GatewayEdgeSecurityError::OriginMismatch);
        }
    }
    if let Some(expected_csrf_token) = policy.expected_csrf_token {
        let csrf = unique_header(headers, "x-csrf-token")
            .ok_or(GatewayEdgeSecurityError::CsrfMissingOrDuplicated)?;
        if csrf != expected_csrf_token {
            return Err(GatewayEdgeSecurityError::CsrfMismatch);
        }
    }
    Ok(())
}

fn is_forwarded_header(header: &GatewayHttpHeader) -> bool {
    matches!(
        header.normalized_name().as_str(),
        "forwarded" | "x-forwarded-for" | "x-forwarded-host" | "x-forwarded-proto" | "x-real-ip"
    )
}

fn unique_header<'a>(headers: &'a [GatewayHttpHeader], name: &str) -> Option<&'a str> {
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == name)
        .map(|header| header.value.as_str());
    let value = values.next()?;
    values.next().is_none().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers() -> Vec<GatewayHttpHeader> {
        vec![
            GatewayHttpHeader::new("Host", "gateway.example.com"),
            GatewayHttpHeader::new("Origin", "https://console.example.com"),
            GatewayHttpHeader::new("X-CSRF-Token", "synthetic-csrf-token"),
        ]
    }

    fn policy() -> GatewayEdgeSecurityPolicy<'static> {
        GatewayEdgeSecurityPolicy {
            peer_is_trusted_proxy: false,
            expected_host: "gateway.example.com",
            expected_origin: Some("https://console.example.com"),
            expected_csrf_token: Some("synthetic-csrf-token"),
        }
    }

    #[test]
    fn edge_security_rejects_forwarding_spoof_and_host_origin_csrf_mismatch() {
        for name in ["X-Forwarded-For", "X-Real-IP"] {
            let mut spoofed = headers();
            spoofed.push(GatewayHttpHeader::new(name, "198.51.100.7"));
            assert_eq!(
                validate_gateway_edge_security(policy(), &spoofed),
                Err(GatewayEdgeSecurityError::ForwardedHeaderFromUntrustedPeer)
            );
        }

        for (name, value, expected) in [
            (
                "Host",
                "evil.example.com",
                GatewayEdgeSecurityError::HostMismatch,
            ),
            (
                "Origin",
                "https://evil.example.com",
                GatewayEdgeSecurityError::OriginMismatch,
            ),
            (
                "X-CSRF-Token",
                "wrong-token",
                GatewayEdgeSecurityError::CsrfMismatch,
            ),
        ] {
            let mut candidate = headers();
            candidate
                .iter_mut()
                .find(|header| header.normalized_name() == name.to_ascii_lowercase())
                .unwrap()
                .value = value.to_string();
            assert_eq!(
                validate_gateway_edge_security(policy(), &candidate),
                Err(expected)
            );
        }

        let mut duplicate_host = headers();
        duplicate_host.push(GatewayHttpHeader::new("Host", "gateway.example.com"));
        assert_eq!(
            validate_gateway_edge_security(policy(), &duplicate_host),
            Err(GatewayEdgeSecurityError::HostMissingOrDuplicated)
        );
    }
}
