#![no_main]

use libfuzzer_sys::fuzz_target;
use prodex_authn::{OidcEndpointPolicy, ValidatedOidcIssuer};

fuzz_target!(|input: &[u8]| {
    let Ok(raw) = std::str::from_utf8(input) else {
        return;
    };
    let mut parts = raw.splitn(3, '\0');
    let issuer = parts.next().unwrap_or_default();
    let jwks = parts.next().filter(|value| !value.is_empty());
    let allowed_origin = parts.next().filter(|value| !value.is_empty());
    let allowlist = allowed_origin.into_iter();

    let Ok(policy) = OidcEndpointPolicy::with_jwks_origin_allowlist(issuer, jwks, allowlist) else {
        return;
    };

    let reparsed = ValidatedOidcIssuer::parse(policy.issuer().as_str()).unwrap();
    assert_eq!(&reparsed, policy.issuer());
    assert!(!policy.redirects_allowed());
    assert!(
        policy
            .validate_redirect("https://example.com/keys")
            .is_err()
    );

    let discovery = policy.discovery_endpoint();
    assert!(policy.validate_jwks_endpoint(discovery.as_str()).is_ok());
    let debug = format!("{policy:?}");
    assert!(!debug.contains(policy.issuer().as_str()));
    if let Some(jwks) = policy.configured_jwks() {
        assert!(!debug.contains(jwks.as_str()));
    }
});
