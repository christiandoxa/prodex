use std::path::{Path, PathBuf};
use std::time::Duration;

use postgres::config::SslMode;
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, pem::PemObject},
};
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::PostgresRuntimeError;

const POSTGRES_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const POSTGRES_STATEMENT_TIMEOUT_MS: u64 = 120_000;
const POSTGRES_LOCK_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostgresTlsMode {
    VerifyFull,
    Disable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresTlsConfig {
    mode: PostgresTlsMode,
    ca_path: Option<PathBuf>,
}

impl PostgresTlsConfig {
    pub fn verify_full(ca_path: Option<PathBuf>) -> Self {
        Self {
            mode: PostgresTlsMode::VerifyFull,
            ca_path,
        }
    }

    pub fn explicit_disable() -> Self {
        Self {
            mode: PostgresTlsMode::Disable,
            ca_path: None,
        }
    }

    pub fn mode(&self) -> PostgresTlsMode {
        self.mode
    }

    pub fn ca_path(&self) -> Option<&Path> {
        self.ca_path.as_deref()
    }

    pub(crate) fn rustls_connector(&self) -> Result<MakeRustlsConnect, PostgresRuntimeError> {
        let native = rustls_native_certs::load_native_certs();
        let mut roots = RootCertStore::empty();
        roots.add_parsable_certificates(native.certs);
        if let Some(path) = self.ca_path() {
            let certs = CertificateDer::pem_file_iter(path)
                .map_err(|_| PostgresRuntimeError::Configuration)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| PostgresRuntimeError::Configuration)?;
            if certs.is_empty() {
                return Err(PostgresRuntimeError::Configuration);
            }
            for cert in certs {
                roots
                    .add(cert)
                    .map_err(|_| PostgresRuntimeError::Configuration)?;
            }
        }
        if roots.is_empty() {
            return Err(PostgresRuntimeError::Configuration);
        }
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(MakeRustlsConnect::new(config))
    }
}

pub fn connect_blocking(
    database_url: &str,
    tls: &PostgresTlsConfig,
) -> Result<postgres::Client, PostgresRuntimeError> {
    let mut config = bounded_postgres_config(database_url)?;
    match tls.mode() {
        PostgresTlsMode::Disable => config
            .ssl_mode(SslMode::Disable)
            .connect(postgres::NoTls)
            .map_err(|_| PostgresRuntimeError::Database),
        PostgresTlsMode::VerifyFull => config
            .ssl_mode(SslMode::Require)
            .connect(tls.rustls_connector()?)
            .map_err(|_| PostgresRuntimeError::Database),
    }
}

fn bounded_postgres_config(database_url: &str) -> Result<postgres::Config, PostgresRuntimeError> {
    let mut config: postgres::Config = database_url
        .parse()
        .map_err(|_| PostgresRuntimeError::Configuration)?;
    if config.get_connect_timeout().is_none() {
        config.connect_timeout(POSTGRES_CONNECT_TIMEOUT);
    }
    let existing = config.get_options().unwrap_or_default();
    let mut options = existing.to_string();
    if !existing.contains("statement_timeout") {
        options.push_str(&format!(
            " -c statement_timeout={POSTGRES_STATEMENT_TIMEOUT_MS}"
        ));
    }
    if !existing.contains("lock_timeout") {
        options.push_str(&format!(" -c lock_timeout={POSTGRES_LOCK_TIMEOUT_MS}"));
    }
    config.options(options.trim());
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_disable_never_accepts_a_ca_path() {
        let config = PostgresTlsConfig::explicit_disable();
        assert_eq!(config.mode(), PostgresTlsMode::Disable);
        assert_eq!(config.ca_path(), None);
    }

    #[test]
    fn verify_full_rejects_a_missing_custom_ca() {
        let config = PostgresTlsConfig::verify_full(Some(PathBuf::from(
            "/path/that/does/not/exist/prodex-ca.pem",
        )));
        assert!(matches!(
            config.rustls_connector(),
            Err(PostgresRuntimeError::Configuration)
        ));
    }

    #[test]
    fn blocking_connections_have_default_deadlines() {
        let config = bounded_postgres_config("postgresql://test@localhost/prodex").unwrap();

        assert_eq!(
            config.get_connect_timeout(),
            Some(&POSTGRES_CONNECT_TIMEOUT)
        );
        let options = config.get_options().unwrap();
        assert!(options.contains("statement_timeout=120000"));
        assert!(options.contains("lock_timeout=30000"));
    }
}
