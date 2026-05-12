use super::*;

#[test]
fn runtime_doctor_degraded_routes_sort_and_cap_output() {
    let now = Local::now().timestamp();
    let routes = runtime_doctor_degraded_routes(
        &RuntimeProfileBackoffs {
            retry_backoff_until: BTreeMap::from([
                ("alpha".to_string(), now + 10),
                ("zeta".to_string(), now + 11),
            ]),
            transport_backoff_until: BTreeMap::from([
                (
                    runtime_profile_transport_backoff_key("beta", RuntimeRouteKind::Responses),
                    now + 20,
                ),
                ("gamma".to_string(), now + 21),
            ]),
            route_circuit_open_until: BTreeMap::from([
                ("__route_circuit__:responses:delta".to_string(), now - 1),
                ("__route_circuit__:websocket:eta".to_string(), now + 30),
                ("__route_circuit__:compact:theta".to_string(), now + 31),
            ]),
        },
        &BTreeMap::from([
            (
                "__route_bad_pairing__:standard:aardvark".to_string(),
                RuntimeProfileHealth {
                    score: 5,
                    updated_at: now,
                },
            ),
            (
                "__route_health__:compact:lambda".to_string(),
                RuntimeProfileHealth {
                    score: 1,
                    updated_at: now,
                },
            ),
            (
                "__route_bad_pairing__:responses:main".to_string(),
                RuntimeProfileHealth {
                    score: 2,
                    updated_at: now,
                },
            ),
            (
                "__route_health__:websocket:omega".to_string(),
                RuntimeProfileHealth {
                    score: 4,
                    updated_at: now,
                },
            ),
        ]),
        now,
    );

    assert_eq!(routes.len(), 8);
    assert_eq!(routes[0], "aardvark/standard bad_pairing=5");
    assert_eq!(
        routes[1],
        format!("alpha/retry retry_backoff until={}", now + 10)
    );
    assert_eq!(
        routes[3],
        format!("delta/responses circuit=half-open until={}", now - 1)
    );
    assert_eq!(
        routes[4],
        format!("eta/websocket circuit=open until={}", now + 30)
    );
    assert_eq!(
        routes[7], "main/responses bad_pairing=2",
        "helper should keep the first eight sorted entries"
    );
    assert!(
        !routes.iter().any(|route| route.starts_with("omega/")
            || route.starts_with("theta/")
            || route.starts_with("zeta/")),
        "later sorted entries should be truncated: {routes:?}"
    );
}
