#![no_main]

use libfuzzer_sys::fuzz_target;
use prodex_gateway_http::{
    CanonicalRequestTarget, GatewayHttpRouteKind, classify_request_target, classify_route,
};

fuzz_target!(|input: &[u8]| {
    let Ok(raw) = std::str::from_utf8(input) else {
        return;
    };
    let Ok(target) = CanonicalRequestTarget::parse(raw) else {
        return;
    };

    assert_eq!(target.path_and_query(), raw);
    assert_eq!(
        CanonicalRequestTarget::parse(target.path_and_query()).unwrap(),
        target
    );

    let borrowed = classify_request_target(&target);
    let borrowed_kind = borrowed.map_or(GatewayHttpRouteKind::Unknown, |route| route.kind);
    assert_eq!(classify_route(raw), borrowed_kind);
    if let Some(route) = borrowed {
        assert_eq!(route.plane, route.kind.plane().unwrap());
    }
});
