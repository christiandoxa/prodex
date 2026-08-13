use std::{error::Error, fmt, net::SocketAddr};

use bytes::Bytes;
use http_body_util::BodyExt as _;
use hyper::{
    Request, Response, StatusCode,
    body::Body,
    header::{CONTENT_LENGTH, HeaderName, HeaderValue},
    upgrade,
};
use prodex_gateway_http::{CanonicalRequestTarget, GatewayHttpRouteKind};

use super::{
    GatewayBoxError, GatewayInProcessUpgradeHandoff, GatewayRequestBody, GatewayResponseBody,
};

/// Canonical, route-classified request delivered to an in-process gateway handler.
pub struct GatewayHandlerRequest {
    pub peer_addr: SocketAddr,
    pub client_ip: std::net::IpAddr,
    pub peer_is_trusted_proxy: bool,
    pub mtls_peer_certificate_sha256: Option<[u8; 32]>,
    pub target: CanonicalRequestTarget,
    pub route: GatewayHttpRouteKind,
    pub request: Request<GatewayRequestBody>,
}

/// Streaming response returned by an in-process gateway handler.
pub struct GatewayHandlerResponse {
    pub response: Response<GatewayResponseBody>,
    pub backend_upgrade: Option<GatewayHandlerUpgrade>,
}

pub enum GatewayHandlerUpgrade {
    Backend(upgrade::OnUpgrade),
    InProcess(GatewayInProcessUpgradeHandoff),
}

impl GatewayHandlerResponse {
    pub fn new<B>(response: Response<B>) -> Self
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Error + Send + Sync + 'static,
    {
        Self {
            response: response.map(|body| {
                body.map_err(|error| Box::new(error) as GatewayBoxError)
                    .boxed_unsync()
            }),
            backend_upgrade: None,
        }
    }

    pub fn with_backend_upgrade<B>(
        response: Response<B>,
        backend_upgrade: upgrade::OnUpgrade,
    ) -> Self
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Error + Send + Sync + 'static,
    {
        let mut handled = Self::new(response);
        handled.backend_upgrade = Some(GatewayHandlerUpgrade::Backend(backend_upgrade));
        handled
    }

    pub fn with_in_process_upgrade<B>(
        response: Response<B>,
        upgrade: GatewayInProcessUpgradeHandoff,
    ) -> Self
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Error + Send + Sync + 'static,
    {
        let mut handled = Self::new(response);
        handled.backend_upgrade = Some(GatewayHandlerUpgrade::InProcess(upgrade));
        handled
    }

    pub fn from_parts(
        status: u16,
        headers: Vec<(String, Vec<u8>)>,
        content_length: Option<usize>,
        body: GatewayResponseBody,
    ) -> GatewayHandlerResult {
        let mut response = Response::new(body);
        *response.status_mut() =
            StatusCode::from_u16(status).map_err(|_| GatewayHandlerError::Unavailable)?;
        for (name, value) in headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| GatewayHandlerError::Unavailable)?;
            let value =
                HeaderValue::from_bytes(&value).map_err(|_| GatewayHandlerError::Unavailable)?;
            response.headers_mut().append(name, value);
        }
        if let Some(content_length) = content_length {
            response.headers_mut().insert(
                CONTENT_LENGTH,
                HeaderValue::from_str(&content_length.to_string())
                    .map_err(|_| GatewayHandlerError::Unavailable)?,
            );
        }
        Ok(Self {
            response,
            backend_upgrade: None,
        })
    }

    pub fn with_in_process_upgrade_handoff(
        mut self,
        upgrade: GatewayInProcessUpgradeHandoff,
    ) -> Self {
        self.backend_upgrade = Some(GatewayHandlerUpgrade::InProcess(upgrade));
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHandlerError {
    InvalidRequest,
    InvalidRequestTarget,
    RequestBodyTooLarge,
    Overloaded,
    Unavailable,
}

impl fmt::Display for GatewayHandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gateway handler failed")
    }
}

impl Error for GatewayHandlerError {}

pub type GatewayHandlerResult = std::result::Result<GatewayHandlerResponse, GatewayHandlerError>;
