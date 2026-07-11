use prodex_runtime_state::RuntimeRouteKind;
use runtime_proxy_crate as runtime_proxy;

pub fn runtime_route_kind_to_proxy(
    route_kind: RuntimeRouteKind,
) -> runtime_proxy::RuntimeRouteKind {
    match route_kind {
        RuntimeRouteKind::Responses => runtime_proxy::RuntimeRouteKind::Responses,
        RuntimeRouteKind::Compact => runtime_proxy::RuntimeRouteKind::Compact,
        RuntimeRouteKind::Websocket => runtime_proxy::RuntimeRouteKind::Websocket,
        RuntimeRouteKind::Standard => runtime_proxy::RuntimeRouteKind::Standard,
    }
}
