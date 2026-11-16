//! Startup-only trust configuration shared by upstream HTTP and WebSocket transports.
//! Explicit certificate files extend platform roots; verification is never disabled.

use anyhow::{Context, Result, bail};
use rustls::pki_types::{CertificateDer, pem::PemObject};
use std::ffi::OsString;
use std::io::Read;
use std::sync::Arc;

const MAX_CA_BUNDLE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Default)]
pub(crate) struct UpstreamTlsConfig {
    certificates: Vec<reqwest::Certificate>,
    websocket: Option<Arc<rustls::ClientConfig>>,
}

impl UpstreamTlsConfig {
    pub(crate) fn from_process() -> Result<Self> {
        Self::from_values(
            std::env::var_os("CODEX_CA_CERTIFICATE"),
            std::env::var_os("SSL_CERT_FILE"),
            false,
        )
    }

    pub(crate) fn from_values(
        codex_ca: Option<OsString>,
        ssl_ca: Option<OsString>,
        proxy_tls: bool,
    ) -> Result<Self> {
        let selected = codex_ca
            .filter(|path| !path.is_empty())
            .map(|path| ("CODEX_CA_CERTIFICATE", path))
            .or_else(|| {
                ssl_ca
                    .filter(|path| !path.is_empty())
                    .map(|path| ("SSL_CERT_FILE", path))
            });
        if selected.is_none() && !proxy_tls {
            return Ok(Self::default());
        }
        let mut roots = rustls::RootCertStore::empty();
        let native = rustls_native_certs::load_native_certs();
        roots.add_parsable_certificates(native.certs);
        let mut certificates = Vec::new();
        if let Some((key, path)) = selected {
            if !std::fs::metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
                bail!("{key} must select a readable regular PEM certificate file");
            }
            let file = std::fs::File::open(path)
                .map_err(|_| anyhow::anyhow!("{key} certificate bundle could not be opened"))?;
            if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
                bail!("{key} must select a regular PEM certificate file");
            }
            let mut bytes = Vec::new();
            file.take(MAX_CA_BUNDLE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| anyhow::anyhow!("{key} certificate bundle could not be read"))?;
            if bytes.len() as u64 > MAX_CA_BUNDLE_BYTES {
                bail!("{key} certificate bundle exceeds the 4 MiB limit");
            }
            for certificate in CertificateDer::pem_slice_iter(&bytes) {
                let certificate = certificate
                    .map_err(|_| anyhow::anyhow!("{key} contains malformed PEM certificates"))?;
                roots
                    .add(certificate.clone())
                    .map_err(|_| anyhow::anyhow!("{key} contains an invalid root certificate"))?;
                certificates.push(
                    reqwest::Certificate::from_der(certificate.as_ref()).map_err(|_| {
                        anyhow::anyhow!("{key} contains an invalid HTTP root certificate")
                    })?,
                );
            }
            if certificates.is_empty() {
                bail!("{key} contains no PEM certificates");
            }
        }
        if roots.is_empty() {
            bail!("upstream TLS has no usable certificate authorities");
        }
        let websocket = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .context("failed to configure upstream TLS protocol versions")?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok(Self {
            certificates,
            websocket: Some(Arc::new(websocket)),
        })
    }

    pub(crate) fn http_certificates(&self) -> impl Iterator<Item = reqwest::Certificate> + '_ {
        self.certificates.iter().cloned()
    }

    pub(crate) fn websocket_connector(&self) -> Option<tungstenite::Connector> {
        self.websocket
            .as_ref()
            .map(|config| tungstenite::Connector::Rustls(Arc::clone(config)))
    }

    pub(crate) fn proxy_client_config(&self) -> Result<Arc<rustls::ClientConfig>> {
        self.websocket
            .clone()
            .context("HTTPS proxy TLS configuration was not prepared at startup")
    }
}

#[cfg(test)]
#[path = "tls_tests.rs"]
mod tests;
