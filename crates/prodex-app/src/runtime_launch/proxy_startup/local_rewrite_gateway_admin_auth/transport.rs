use super::super::*;
use super::endpoint_policy::RuntimeGatewayOidcEndpoint;
use prodex_authn::ValidatedOidcEndpoint;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

const MAX_RUNTIME_GATEWAY_OIDC_RESOLVED_ADDRESSES: usize = 16;
static RUNTIME_GATEWAY_OIDC_DNS_RESOLUTION_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

struct RuntimeGatewayOidcDnsResolutionGuard;

impl Drop for RuntimeGatewayOidcDnsResolutionGuard {
    fn drop(&mut self) {
        RUNTIME_GATEWAY_OIDC_DNS_RESOLUTION_IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

pub(super) fn runtime_gateway_oidc_send(
    endpoint: &RuntimeGatewayOidcEndpoint,
    description: &str,
    timing: &RuntimeOidcTimingConfig,
) -> Result<reqwest::blocking::Response> {
    match endpoint {
        RuntimeGatewayOidcEndpoint::Validated(endpoint) => {
            let timeout = timing.prefetch_timeout;
            let resolved_addresses = runtime_gateway_oidc_resolve(endpoint, timeout)?;
            let client = reqwest::blocking::Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(timeout)
                .timeout(timeout)
                .no_proxy()
                .resolve_to_addrs(endpoint.host(), &resolved_addresses)
                .build()
                .context("failed to build gateway OIDC HTTP client")?;
            let response = client
                .get(endpoint.as_str())
                .send()
                .with_context(|| format!("failed to fetch gateway OIDC {description}"))?;
            let remote = response
                .remote_addr()
                .ok_or_else(|| anyhow::anyhow!("gateway OIDC peer address is unavailable"))?;
            if !runtime_gateway_oidc_peer_is_allowed(remote, &resolved_addresses) {
                bail!("gateway OIDC connected peer is not allowed");
            }
            if response.url().as_str() != endpoint.as_str() {
                bail!("gateway OIDC redirects are forbidden");
            }
            if response.status().is_redirection() {
                bail!("gateway OIDC redirects are forbidden");
            }
            response
                .error_for_status()
                .with_context(|| format!("gateway OIDC {description} endpoint returned an error"))
        }
        #[cfg(test)]
        RuntimeGatewayOidcEndpoint::InsecureLoopback(endpoint) => {
            let response = reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(timing.prefetch_timeout)
                .timeout(timing.prefetch_timeout)
                .no_proxy()
                .build()
                .context("failed to build test OIDC HTTP client")?
                .get(endpoint)
                .send()
                .with_context(|| format!("failed to fetch test OIDC {description}"))?;
            if response.status().is_redirection() || response.url().as_str() != endpoint {
                bail!("test OIDC redirects are forbidden");
            }
            response
                .error_for_status()
                .with_context(|| format!("test OIDC {description} endpoint returned an error"))
        }
    }
}

pub(in super::super) fn runtime_gateway_oidc_post_form(
    endpoint: &RuntimeGatewayOidcEndpoint,
    form: &[(String, String)],
    timing: &RuntimeOidcTimingConfig,
) -> Result<reqwest::blocking::Response> {
    match endpoint {
        RuntimeGatewayOidcEndpoint::Validated(endpoint) => {
            let timeout = timing.prefetch_timeout;
            let resolved_addresses = runtime_gateway_oidc_resolve(endpoint, timeout)?;
            let client = reqwest::blocking::Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(timeout)
                .timeout(timeout)
                .no_proxy()
                .resolve_to_addrs(endpoint.host(), &resolved_addresses)
                .build()
                .context("failed to build gateway OIDC token client")?;
            let response = client
                .post(endpoint.as_str())
                .form(form)
                .send()
                .context("gateway OIDC token exchange failed")?;
            let remote = response
                .remote_addr()
                .ok_or_else(|| anyhow::anyhow!("gateway OIDC peer address is unavailable"))?;
            if !runtime_gateway_oidc_peer_is_allowed(remote, &resolved_addresses)
                || response.url().as_str() != endpoint.as_str()
                || response.status().is_redirection()
            {
                bail!("gateway OIDC token endpoint transport is not permitted");
            }
            response
                .error_for_status()
                .context("gateway OIDC token endpoint returned an error")
        }
        #[cfg(test)]
        RuntimeGatewayOidcEndpoint::InsecureLoopback(endpoint) => {
            let response = reqwest::blocking::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(timing.prefetch_timeout)
                .timeout(timing.prefetch_timeout)
                .no_proxy()
                .build()
                .context("failed to build test OIDC token client")?
                .post(endpoint)
                .form(form)
                .send()
                .context("test OIDC token exchange failed")?;
            if response.status().is_redirection() || response.url().as_str() != endpoint {
                bail!("test OIDC token endpoint redirects are forbidden");
            }
            response
                .error_for_status()
                .context("test OIDC token endpoint returned an error")
        }
    }
}

pub(super) fn runtime_gateway_oidc_resolve(
    endpoint: &ValidatedOidcEndpoint,
    timeout: Duration,
) -> Result<Vec<SocketAddr>> {
    if RUNTIME_GATEWAY_OIDC_DNS_RESOLUTION_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        bail!("gateway OIDC DNS resolution concurrency limit reached");
    }
    let host = endpoint.host().to_string();
    let port = endpoint.port();
    let (sender, receiver) = mpsc::sync_channel(1);
    let resolution_guard = RuntimeGatewayOidcDnsResolutionGuard;
    std::thread::Builder::new()
        .name("prodex-oidc-dns".to_string())
        .spawn(move || {
            let _guard = resolution_guard;
            let result = (host.as_str(), port).to_socket_addrs().map(|addresses| {
                addresses
                    .take(MAX_RUNTIME_GATEWAY_OIDC_RESOLVED_ADDRESSES + 1)
                    .collect::<Vec<_>>()
            });
            let _ = sender.send(result);
        })
        .context("failed to start gateway OIDC DNS resolver")?;
    let mut addresses = receiver
        .recv_timeout(timeout)
        .context("gateway OIDC DNS resolution timed out")?
        .context("failed to resolve gateway OIDC endpoint")?;
    if addresses.is_empty() {
        bail!("gateway OIDC endpoint resolved to no addresses");
    }
    if addresses.len() > MAX_RUNTIME_GATEWAY_OIDC_RESOLVED_ADDRESSES {
        bail!("gateway OIDC endpoint resolved to too many addresses");
    }
    if addresses
        .iter()
        .any(|address| runtime_gateway_oidc_ip_is_forbidden(address.ip()))
    {
        bail!("gateway OIDC endpoint resolved to a forbidden address");
    }
    addresses.sort_unstable();
    addresses.dedup();
    Ok(addresses)
}

pub(super) fn runtime_gateway_oidc_peer_is_allowed(
    remote: SocketAddr,
    resolved_addresses: &[SocketAddr],
) -> bool {
    !runtime_gateway_oidc_ip_is_forbidden(remote.ip()) && resolved_addresses.contains(&remote)
}

pub(super) fn runtime_gateway_oidc_ip_is_forbidden(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let [first, second, third, _] = address.octets();
            first == 0
                || first == 10
                || (first == 100 && (64..=127).contains(&second))
                || first == 127
                || (first == 169 && second == 254)
                || (first == 172 && (16..=31).contains(&second))
                || (first == 192 && (second == 168 || (second == 0 && matches!(third, 0 | 2))))
                || (first == 198 && ((18..=19).contains(&second) || (second == 51 && third == 100)))
                || (first == 203 && second == 0 && third == 113)
                || first >= 224
        }
        IpAddr::V6(address) => {
            let octets = address.octets();
            address.is_unspecified()
                || address.is_loopback()
                || address.is_multicast()
                || octets[0] & 0xfe == 0xfc
                || (octets[0] == 0xfe && octets[1] & 0xc0 == 0x80)
                || (octets[0] == 0x20
                    && octets[1] == 0x01
                    && octets[2] == 0x0d
                    && octets[3] == 0xb8)
                || address
                    .to_ipv4_mapped()
                    .is_some_and(|address| runtime_gateway_oidc_ip_is_forbidden(address.into()))
                || ((octets[..12] == [0; 12]
                    || octets[..12] == [0, 0x64, 0xff, 0x9b, 0, 0, 0, 0, 0, 0, 0, 0])
                    && runtime_gateway_oidc_ip_is_forbidden(
                        std::net::Ipv4Addr::new(octets[12], octets[13], octets[14], octets[15])
                            .into(),
                    ))
                || octets[..6] == [0, 0x64, 0xff, 0x9b, 0, 1]
                || octets[..4] == [0x20, 0x01, 0, 0]
                || (octets[..2] == [0x20, 0x02]
                    && runtime_gateway_oidc_ip_is_forbidden(
                        std::net::Ipv4Addr::new(octets[2], octets[3], octets[4], octets[5]).into(),
                    ))
        }
    }
}
