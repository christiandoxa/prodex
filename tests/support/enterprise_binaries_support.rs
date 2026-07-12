pub(super) fn bin(name: &str) -> String {
    std::env::var(format!("CARGO_BIN_EXE_{name}")).expect("cargo should expose binary path")
}

pub(super) fn closed_otlp_endpoint() -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind OTLP closed-port listener");
    let addr = listener
        .local_addr()
        .expect("resolve OTLP closed-port listener addr");
    drop(listener);
    format!("http://{addr}/v1/logs")
}

macro_rules! assert_scoped_idempotency_key {
    ($stdout:expr, $principal_suffix:literal, $presented_key:literal) => {{
        let principal_id = format!(
            "00000000-0000-7000-8000-{suffix:012}",
            suffix = $principal_suffix,
        )
        .parse()
        .expect("principal id should parse");
        let presented_key = prodex_domain::IdempotencyKey::new($presented_key)
            .expect("presented idempotency key should be valid");
        let expected = prodex_domain::IdempotencyKey::from_control_plane_principal(
            principal_id,
            &presented_key,
        );
        assert_eq!(
            $stdout["idempotency"]["key"].as_str(),
            Some(expected.as_str())
        );
    }};
}
