use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use postgres::config::SslMode;
use rustls::{ClientConfig, RootCertStore};
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::PostgresRuntimeError;

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
            let file = File::open(path).map_err(|_| PostgresRuntimeError::Configuration)?;
            let mut reader = BufReader::new(file);
            let certs = rustls_pemfile::certs(&mut reader)
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
    let mut config: postgres::Config = database_url
        .parse()
        .map_err(|_| PostgresRuntimeError::Configuration)?;
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
}
