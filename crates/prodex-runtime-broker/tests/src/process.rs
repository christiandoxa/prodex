use std::ffi::OsString;
use std::io::{Cursor, Write};
use std::path::PathBuf;

use super::*;

#[test]
fn broker_process_plan_exposes_no_bootstrap_fields_or_secrets() {
    let admin_token = RuntimeBrokerSecret::new("admin-token").unwrap();
    let config = RuntimeBrokerSpawnConfig {
        current_profile: "default",
        upstream_base_url: "https://upstream.example",
        include_code_review: true,
        upstream_no_proxy: true,
        smart_context_enabled: true,
        model_context_window_tokens: Some(65_536),
        broker_key: "key",
        instance_id: "broker-token",
        admin_token: &admin_token,
        listen_addr: Some("127.0.0.1:4567"),
    };
    let args = runtime_broker_process_args();

    assert_eq!(args, vec![OsString::from("__runtime-broker")]);

    let plan = runtime_broker_process_command_plan("/bin/prodex", "/tmp/prodex-home");

    assert_eq!(plan.executable, PathBuf::from("/bin/prodex"));
    assert_eq!(
        plan.environment,
        vec![(
            OsString::from("PRODEX_HOME"),
            OsString::from("/tmp/prodex-home")
        )]
    );
    for rendered in [format!("{config:?}"), format!("{admin_token:?}")] {
        assert!(!rendered.contains("admin-token"));
    }
    let rendered_plan = format!("{plan:?}");
    for forbidden in [
        "admin-token",
        "broker-token",
        "admin_token",
        "instance_token",
    ] {
        assert!(
            !rendered_plan.contains(forbidden),
            "broker process plan leaked {forbidden}: {rendered_plan}"
        );
    }
}

#[test]
fn broker_secret_and_capability_errors_do_not_echo_payloads() {
    let secret = RuntimeBrokerSecret::new("raw-capability-sentinel").unwrap();
    assert_eq!(format!("{secret:?}"), "RuntimeBrokerSecret(\"<redacted>\")");

    fn requires_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}
    requires_zeroize_on_drop::<RuntimeBrokerSecret>();

    let malformed = br#"{"version":1,"instance_id":"instance","admin_token":"raw-capability-sentinel",invalid}"#;
    let error = read_runtime_broker_capability(Cursor::new(malformed)).unwrap_err();
    assert_eq!(error, RuntimeBrokerCapabilityError::Invalid);
    assert!(!format!("{error:?} {error}").contains("raw-capability-sentinel"));

    let invalid = RuntimeBrokerSecret::new("").unwrap_err();
    assert_eq!(invalid.to_string(), "runtime broker secret is invalid");
    assert!(!format!("{invalid:?} {invalid}").contains("raw-capability-sentinel"));
}

#[test]
fn broker_bootstrap_round_trips_over_the_bounded_pipe_format() {
    let admin_token = RuntimeBrokerSecret::new("admin-token").unwrap();
    let config = RuntimeBrokerSpawnConfig {
        current_profile: "default",
        upstream_base_url: "https://upstream.example",
        include_code_review: true,
        upstream_no_proxy: true,
        smart_context_enabled: true,
        model_context_window_tokens: Some(65_536),
        broker_key: "key",
        instance_id: "broker-instance",
        admin_token: &admin_token,
        listen_addr: Some("127.0.0.1:4567"),
    };
    let mut payload = Vec::new();

    write_runtime_broker_bootstrap(&mut payload, config).unwrap();
    let bootstrap = read_runtime_broker_bootstrap(Cursor::new(payload)).unwrap();

    assert_eq!(bootstrap.current_profile, "default");
    assert_eq!(bootstrap.instance_id, "broker-instance");
    assert!(bootstrap.admin_token.matches("admin-token"));
    let rendered = format!("{bootstrap:?}");
    assert!(!rendered.contains("admin-token"));
    assert!(rendered.contains("<redacted>"));
}

#[test]
fn broker_bootstrap_rejects_malformed_truncated_and_oversized_inputs_without_echoing_secrets() {
    let malformed = br#"{"admin_token":"do-not-echo",invalid}"#;
    let truncated = br#"{"version":1,"admin_token":"do-not-echo""#;
    for payload in [malformed.as_slice(), truncated.as_slice()] {
        let err = read_runtime_broker_bootstrap(Cursor::new(payload)).unwrap_err();
        assert_eq!(err, RuntimeBrokerBootstrapError::Invalid);
        assert!(!err.to_string().contains("do-not-echo"));
    }

    let oversized = vec![b'x'; RUNTIME_BROKER_BOOTSTRAP_MAX_BYTES + 1];
    let err = read_runtime_broker_bootstrap(Cursor::new(oversized)).unwrap_err();
    assert_eq!(err, RuntimeBrokerBootstrapError::TooLarge);
}

#[test]
fn broker_bootstrap_rejects_unsupported_versions() {
    let payload = br#"{
        "version":2,
        "current_profile":"default",
        "upstream_base_url":"https://upstream.example",
        "include_code_review":false,
        "upstream_no_proxy":false,
        "smart_context_enabled":false,
        "model_context_window_tokens":null,
        "broker_key":"key",
        "instance_id":"instance",
        "admin_token":"do-not-echo",
        "listen_addr":null
    }"#;

    let err = read_runtime_broker_bootstrap(Cursor::new(payload)).unwrap_err();

    assert_eq!(err, RuntimeBrokerBootstrapError::UnsupportedVersion);
    assert!(!err.to_string().contains("do-not-echo"));
}

#[test]
fn broker_bootstrap_write_errors_do_not_echo_the_capability() {
    struct RejectWriter;
    impl Write for RejectWriter {
        fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("rejected"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let admin_token = RuntimeBrokerSecret::new("do-not-echo").unwrap();
    let err = write_runtime_broker_bootstrap(
        RejectWriter,
        RuntimeBrokerSpawnConfig {
            current_profile: "default",
            upstream_base_url: "https://upstream.example",
            include_code_review: false,
            upstream_no_proxy: false,
            smart_context_enabled: false,
            model_context_window_tokens: None,
            broker_key: "key",
            instance_id: "instance",
            admin_token: &admin_token,
            listen_addr: None,
        },
    )
    .unwrap_err();

    assert_eq!(err, RuntimeBrokerBootstrapError::Io);
    assert!(!err.to_string().contains(admin_token.expose()));
}

#[test]
fn broker_capability_envelope_binds_secret_to_instance_and_redacts_debug() {
    let admin_token = RuntimeBrokerSecret::new("bound-admin-token").unwrap();
    let mut payload = Vec::new();

    write_runtime_broker_capability(&mut payload, "broker-instance", &admin_token).unwrap();
    let capability = read_runtime_broker_capability(Cursor::new(payload)).unwrap();

    assert_eq!(capability.instance_id, "broker-instance");
    assert!(capability.admin_token.matches("bound-admin-token"));
    assert!(!format!("{capability:?}").contains("bound-admin-token"));
}

#[test]
fn broker_key_is_scoped_to_smart_context_mode() {
    let normal = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        false,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );
    let smart = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );

    assert_ne!(normal, smart);
}

#[test]
fn broker_key_is_scoped_to_smart_context_window_when_enabled() {
    let default_window = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        None,
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );
    let custom_window = runtime_broker_key_for_binary_identity(
        "https://upstream.example",
        true,
        false,
        true,
        Some(65_536),
        "/backend-api/prodex",
        "version=0.7.0;sha256=abc123",
    );

    assert_ne!(default_window, custom_window);
}
