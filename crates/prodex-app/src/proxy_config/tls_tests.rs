use super::UpstreamTlsConfig;
use std::path::{Path, PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("prodex-ca-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn load(path: &Path) -> anyhow::Result<UpstreamTlsConfig> {
    UpstreamTlsConfig::from_values(Some(path.as_os_str().to_owned()), None, false)
}

#[test]
fn custom_ca_uses_codex_precedence_and_empty_value_fallback() {
    let fixture = Fixture::new();
    let cert = fixture.write(
        "ca.pem",
        include_bytes!("../../tests/fixtures/proxy-ca.pem"),
    );
    let bad = fixture.write("invalid.pem", b"not a certificate");
    let config = UpstreamTlsConfig::from_values(
        Some(cert.as_os_str().to_owned()),
        Some(bad.as_os_str().to_owned()),
        false,
    )
    .unwrap();
    assert_eq!(config.http_certificates().count(), 1);
    assert!(config.websocket_connector().is_some());
    let fallback =
        UpstreamTlsConfig::from_values(Some("".into()), Some(cert.into_os_string()), false)
            .unwrap();
    assert_eq!(fallback.http_certificates().count(), 1);
    assert!(fallback.websocket_connector().is_some());
    let error = UpstreamTlsConfig::from_values(Some(bad.into_os_string()), None, false)
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("CODEX_CA_CERTIFICATE"));
}

#[test]
fn invalid_custom_ca_fails_closed_without_disclosing_the_path_or_contents() {
    let fixture = Fixture::new();
    for bytes in [
        b"PRIVATE-FIXTURE-DATA".as_slice(),
        b"-----BEGIN CERTIFICATE-----\n!bad!\n-----END CERTIFICATE-----\n",
        b"-----BEGIN CERTIFICATE-----\nYWJj\n-----END CERTIFICATE-----\n",
    ] {
        let path = fixture.write("private-fixture-path.pem", bytes);
        let error = load(&path).err().unwrap().to_string();
        assert!(error.contains("CODEX_CA_CERTIFICATE"));
        assert!(!error.contains("private-fixture-path"));
        assert!(!error.contains("PRIVATE-FIXTURE-DATA"));
    }
    assert!(load(&fixture.0).is_err());
    assert!(load(&fixture.0.join("missing-private-path")).is_err());
}

#[test]
fn custom_ca_reads_are_bounded_and_default_keeps_native_connectors() {
    let fixture = Fixture::new();
    let path = fixture.write("large.pem", &vec![b'x'; 4 * 1024 * 1024 + 1]);
    assert!(load(&path).err().unwrap().to_string().contains("4 MiB"));
    let ordinary = UpstreamTlsConfig::from_values(None, None, false).unwrap();
    assert_eq!(ordinary.http_certificates().count(), 0);
    assert!(ordinary.websocket_connector().is_none());
}
