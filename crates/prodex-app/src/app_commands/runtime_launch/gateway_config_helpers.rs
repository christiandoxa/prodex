use super::*;

pub(super) fn gateway_optional_policy_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn gateway_budget_usd_to_microusd(value: f64) -> u64 {
    (value * 1_000_000.0).round().clamp(1.0, u64::MAX as f64) as u64
}

pub(crate) fn gateway_api_keys_from_list(value: &str) -> Option<Vec<String>> {
    let keys = value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    (!keys.is_empty()).then_some(keys)
}

pub(super) fn gateway_validate_listen_auth(listen_addr: &str, auth_required: bool) -> Result<()> {
    let host = listen_addr
        .rsplit_once(':')
        .map(|(host, _)| host.trim_matches(['[', ']']))
        .unwrap_or(listen_addr)
        .trim();
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|addr| addr.is_loopback());
    if !loopback && !auth_required {
        bail!(
            "refusing to bind unauthenticated gateway on non-loopback address {listen_addr}; set --auth-token, PRODEX_GATEWAY_TOKEN, or [[gateway.virtual_keys]]"
        );
    }
    Ok(())
}
