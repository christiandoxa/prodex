use std::{
    fmt,
    net::IpAddr,
    sync::{Arc, RwLock},
};

use anyhow::{Context as _, Result, ensure};
use arc_swap::ArcSwap;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

use super::GatewayServerConfig;

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayServerTlsConfig {
    identity_pem: Vec<u8>,
    client_ca_pem: Option<Vec<u8>>,
    require_client_certificate: bool,
}

impl fmt::Debug for GatewayServerTlsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayServerTlsConfig")
            .field("identity_pem", &"<redacted>")
            .field(
                "client_ca_pem",
                &self.client_ca_pem.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "require_client_certificate",
                &self.require_client_certificate,
            )
            .finish()
    }
}

impl GatewayServerTlsConfig {
    pub fn new(
        identity_pem: Vec<u8>,
        client_ca_pem: Option<Vec<u8>>,
        require_client_certificate: bool,
    ) -> Result<Self> {
        ensure!(!identity_pem.is_empty(), "gateway TLS identity is empty");
        ensure!(
            !require_client_certificate || client_ca_pem.is_some(),
            "gateway mTLS requires a client CA"
        );
        let config = Self {
            identity_pem,
            client_ca_pem,
            require_client_certificate,
        };
        config.server_config()?;
        Ok(config)
    }

    pub(super) fn server_config(&self) -> Result<rustls::ServerConfig> {
        let certificates = CertificateDer::pem_slice_iter(&self.identity_pem)
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to parse gateway TLS certificate chain")?;
        ensure!(
            !certificates.is_empty(),
            "gateway TLS certificate chain is empty"
        );
        let private_key = PrivateKeyDer::from_pem_slice(&self.identity_pem)
            .context("failed to parse gateway TLS private key")?;
        let builder = rustls::ServerConfig::builder();
        let mut server = if let Some(client_ca_pem) = self.client_ca_pem.as_ref() {
            let mut roots = rustls::RootCertStore::empty();
            for certificate in CertificateDer::pem_slice_iter(client_ca_pem) {
                roots
                    .add(certificate.context("failed to parse gateway mTLS client CA")?)
                    .context("failed to load gateway mTLS client CA")?;
            }
            ensure!(!roots.is_empty(), "gateway mTLS client CA is empty");
            let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots));
            let verifier = if self.require_client_certificate {
                verifier.build()
            } else {
                verifier.allow_unauthenticated().build()
            }
            .context("failed to build gateway mTLS client verifier")?;
            builder
                .with_client_cert_verifier(verifier)
                .with_single_cert(certificates, private_key)
        } else {
            builder
                .with_no_client_auth()
                .with_single_cert(certificates, private_key)
        }
        .context("failed to build gateway TLS server configuration")?;
        server.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(server)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayServerEdgeSecurity {
    pub trusted_proxies: Vec<IpAddr>,
    pub expected_host: String,
    pub browser: Option<GatewayServerBrowserSecurity>,
}

impl fmt::Debug for GatewayServerEdgeSecurity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayServerEdgeSecurity")
            .field("trusted_proxies", &self.trusted_proxies)
            .field("expected_host", &"<redacted>")
            .field("browser", &self.browser)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayServerBrowserSecurity {
    pub expected_origin: String,
    pub expected_csrf_token: Option<String>,
}

pub(super) struct GatewayServerRuntimeSecurity {
    pub(super) edge_security: Arc<GatewayServerEdgeSecurity>,
    pub(super) tls_acceptor: Option<tokio_rustls::TlsAcceptor>,
}

#[derive(Clone)]
pub struct GatewayServerReloadHandle {
    pub(super) security: Arc<ArcSwap<GatewayServerRuntimeSecurity>>,
    pub(super) transition: Arc<RwLock<()>>,
}

impl fmt::Debug for GatewayServerReloadHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayServerReloadHandle")
            .finish_non_exhaustive()
    }
}

impl GatewayServerReloadHandle {
    pub fn new(config: &GatewayServerConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            security: Arc::new(ArcSwap::from_pointee(gateway_server_runtime_security(
                config,
            )?)),
            transition: Arc::new(RwLock::new(())),
        })
    }

    pub fn reload(&self, config: &GatewayServerConfig) -> Result<()> {
        self.reload_with_activation(config, || ())
    }

    /// Publishes transport security and dependent handler state under one request barrier.
    pub fn reload_with_activation<T>(
        &self,
        config: &GatewayServerConfig,
        activate: impl FnOnce() -> T,
    ) -> Result<T> {
        config.validate()?;
        let security = Arc::new(gateway_server_runtime_security(config)?);
        let _transition = self
            .transition
            .write()
            .map_err(|_| anyhow::anyhow!("gateway server reload lock is poisoned"))?;
        self.security.store(security);
        Ok(activate())
    }

    pub(super) fn load(&self) -> Arc<GatewayServerRuntimeSecurity> {
        self.security.load_full()
    }
}

pub(super) fn gateway_server_runtime_security(
    config: &GatewayServerConfig,
) -> Result<GatewayServerRuntimeSecurity> {
    let tls_acceptor = config
        .tls
        .as_ref()
        .map(GatewayServerTlsConfig::server_config)
        .transpose()?
        .map(Arc::new)
        .map(tokio_rustls::TlsAcceptor::from);
    Ok(GatewayServerRuntimeSecurity {
        edge_security: Arc::new(config.edge_security.clone()),
        tls_acceptor,
    })
}

impl fmt::Debug for GatewayServerBrowserSecurity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayServerBrowserSecurity")
            .field("expected_origin", &"<redacted>")
            .field("expected_csrf_token", &"<redacted>")
            .finish()
    }
}
