use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpMethod {
    Get,
    Post,
    Patch,
    Delete,
    Options,
    Other,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayHttpHeader {
    pub name: String,
    pub value: String,
}

impl fmt::Debug for GatewayHttpHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayHttpHeader")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("value_length", &self.value.len())
            .finish()
    }
}

impl GatewayHttpHeader {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    pub fn normalized_name(&self) -> String {
        self.name.trim().to_ascii_lowercase()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayHttpRequestMeta {
    pub method: GatewayHttpMethod,
    pub path: String,
    pub body_len: usize,
    pub headers: Vec<GatewayHttpHeader>,
}

impl fmt::Debug for GatewayHttpRequestMeta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayHttpRequestMeta")
            .field("method", &self.method)
            .field("path", &"<redacted>")
            .field("body_len", &self.body_len)
            .field("header_count", &self.headers.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayHttpPolicy {
    pub max_body_bytes: usize,
    pub request_timeout_ms: u64,
    pub stream_idle_timeout_ms: u64,
    pub max_concurrent_streams: u32,
    pub connection_drain_timeout_ms: u64,
    pub require_trace_context: bool,
}

impl GatewayHttpPolicy {
    pub const fn production_default() -> Self {
        Self {
            max_body_bytes: 16 * 1024 * 1024,
            request_timeout_ms: 120_000,
            stream_idle_timeout_ms: 30_000,
            max_concurrent_streams: 1_024,
            connection_drain_timeout_ms: 30_000,
            require_trace_context: true,
        }
    }

    pub fn validate(self) -> Result<(), GatewayHttpPolicyError> {
        if self.max_body_bytes == 0 {
            return Err(GatewayHttpPolicyError::ZeroBodyLimit);
        }
        if self.request_timeout_ms == 0 || self.stream_idle_timeout_ms == 0 {
            return Err(GatewayHttpPolicyError::ZeroTimeout);
        }
        if self.stream_idle_timeout_ms > self.request_timeout_ms {
            return Err(GatewayHttpPolicyError::StreamTimeoutExceedsRequestTimeout);
        }
        if self.max_concurrent_streams == 0 {
            return Err(GatewayHttpPolicyError::ZeroConcurrencyLimit);
        }
        if self.connection_drain_timeout_ms == 0 {
            return Err(GatewayHttpPolicyError::ZeroDrainTimeout);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpPolicyError {
    ZeroBodyLimit,
    ZeroTimeout,
    StreamTimeoutExceedsRequestTimeout,
    ZeroConcurrencyLimit,
    ZeroDrainTimeout,
}

impl fmt::Display for GatewayHttpPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBodyLimit
            | Self::ZeroTimeout
            | Self::StreamTimeoutExceedsRequestTimeout
            | Self::ZeroConcurrencyLimit
            | Self::ZeroDrainTimeout => write!(f, "gateway HTTP policy is invalid"),
        }
    }
}

impl Error for GatewayHttpPolicyError {}
