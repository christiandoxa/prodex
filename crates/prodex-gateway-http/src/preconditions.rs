use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpIdempotencyKeyError {
    Invalid(IdempotencyKeyError),
    Duplicate,
}

impl fmt::Display for GatewayHttpIdempotencyKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Duplicate => write!(f, "request metadata is duplicated"),
        }
    }
}

impl Error for GatewayHttpIdempotencyKeyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpIdempotencyKeyErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpIdempotencyKeyErrorResponsePlan {
    pub status: GatewayHttpIdempotencyKeyErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpEntityTagError {
    Invalid(EntityTagError),
    Duplicate,
}

impl fmt::Display for GatewayHttpEntityTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(error) => error.fmt(f),
            Self::Duplicate => write!(f, "request metadata is duplicated"),
        }
    }
}

impl Error for GatewayHttpEntityTagError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpEntityTagErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpEntityTagErrorResponsePlan {
    pub status: GatewayHttpEntityTagErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpPaginationQueryError {
    Cursor(CursorError),
    InvalidLimit,
    DuplicateLimit,
    DuplicateCursor,
}

impl fmt::Display for GatewayHttpPaginationQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cursor(error) => error.fmt(f),
            Self::InvalidLimit => write!(f, "pagination metadata is invalid"),
            Self::DuplicateLimit | Self::DuplicateCursor => {
                write!(f, "pagination metadata is duplicated")
            }
        }
    }
}

impl Error for GatewayHttpPaginationQueryError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpPaginationQueryErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpPaginationQueryErrorResponsePlan {
    pub status: GatewayHttpPaginationQueryErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_http_pagination_query_error_response(
    error: &GatewayHttpPaginationQueryError,
) -> GatewayHttpPaginationQueryErrorResponsePlan {
    match error {
        GatewayHttpPaginationQueryError::Cursor(error) => {
            let response = plan_cursor_error_response(error);
            GatewayHttpPaginationQueryErrorResponsePlan {
                status: match response.status {
                    CursorErrorStatus::BadRequest => {
                        GatewayHttpPaginationQueryErrorStatus::BadRequest
                    }
                },
                code: response.code,
                message: response.message,
            }
        }
        GatewayHttpPaginationQueryError::InvalidLimit => {
            GatewayHttpPaginationQueryErrorResponsePlan {
                status: GatewayHttpPaginationQueryErrorStatus::BadRequest,
                code: "pagination_limit_invalid",
                message: "pagination limit is invalid",
            }
        }
        GatewayHttpPaginationQueryError::DuplicateLimit => {
            GatewayHttpPaginationQueryErrorResponsePlan {
                status: GatewayHttpPaginationQueryErrorStatus::BadRequest,
                code: "pagination_limit_invalid",
                message: "pagination limit is invalid",
            }
        }
        GatewayHttpPaginationQueryError::DuplicateCursor => {
            GatewayHttpPaginationQueryErrorResponsePlan {
                status: GatewayHttpPaginationQueryErrorStatus::BadRequest,
                code: "pagination_cursor_invalid",
                message: "pagination cursor is invalid",
            }
        }
    }
}

pub fn plan_gateway_http_entity_tag_error_response(
    error: &GatewayHttpEntityTagError,
) -> GatewayHttpEntityTagErrorResponsePlan {
    match error {
        GatewayHttpEntityTagError::Invalid(error) => {
            let response = plan_entity_tag_error_response(error);
            GatewayHttpEntityTagErrorResponsePlan {
                status: GatewayHttpEntityTagErrorStatus::BadRequest,
                code: response.code,
                message: response.message,
            }
        }
        GatewayHttpEntityTagError::Duplicate => GatewayHttpEntityTagErrorResponsePlan {
            status: GatewayHttpEntityTagErrorStatus::BadRequest,
            code: "entity_tag_invalid",
            message: "entity tag is invalid",
        },
    }
}

pub fn plan_gateway_http_idempotency_key_error_response(
    error: &GatewayHttpIdempotencyKeyError,
) -> GatewayHttpIdempotencyKeyErrorResponsePlan {
    match error {
        GatewayHttpIdempotencyKeyError::Invalid(error) => {
            let response = plan_idempotency_key_error_response(error);
            GatewayHttpIdempotencyKeyErrorResponsePlan {
                status: GatewayHttpIdempotencyKeyErrorStatus::BadRequest,
                code: response.code,
                message: response.message,
            }
        }
        GatewayHttpIdempotencyKeyError::Duplicate => GatewayHttpIdempotencyKeyErrorResponsePlan {
            status: GatewayHttpIdempotencyKeyErrorStatus::BadRequest,
            code: "idempotency_key_invalid",
            message: "idempotency key is invalid",
        },
    }
}

pub fn idempotency_key_from_headers(
    headers: &[GatewayHttpHeader],
) -> Result<Option<IdempotencyKey>, GatewayHttpIdempotencyKeyError> {
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == "idempotency-key");
    let Some(header) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(GatewayHttpIdempotencyKeyError::Duplicate);
    }
    IdempotencyKey::new(&header.value)
        .map(Some)
        .map_err(GatewayHttpIdempotencyKeyError::Invalid)
}

pub fn entity_tag_from_if_match_headers(
    headers: &[GatewayHttpHeader],
) -> Result<Option<EntityTag>, GatewayHttpEntityTagError> {
    let mut values = headers
        .iter()
        .filter(|header| header.normalized_name() == "if-match");
    let Some(header) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(GatewayHttpEntityTagError::Duplicate);
    }
    EntityTag::new(&header.value)
        .map(Some)
        .map_err(GatewayHttpEntityTagError::Invalid)
}

pub fn page_request_from_query(
    query: &str,
) -> Result<PageRequest, GatewayHttpPaginationQueryError> {
    let mut limit = None;
    let mut cursor = None;
    for pair in query
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
    {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        match percent_decode_query_name(name).as_ref() {
            "limit" => {
                if limit.is_some() {
                    return Err(GatewayHttpPaginationQueryError::DuplicateLimit);
                }
                limit = Some(
                    value
                        .parse::<u16>()
                        .map_err(|_| GatewayHttpPaginationQueryError::InvalidLimit)?,
                );
            }
            "cursor" => {
                if cursor.is_some() {
                    return Err(GatewayHttpPaginationQueryError::DuplicateCursor);
                }
                cursor = Some(
                    Cursor::new(value.to_string())
                        .map_err(GatewayHttpPaginationQueryError::Cursor)?,
                );
            }
            _ => {}
        }
    }
    Ok(PageRequest::new(limit, cursor))
}

fn percent_decode_query_name(name: &str) -> Cow<'_, str> {
    if !name.as_bytes().contains(&b'%') {
        return Cow::Borrowed(name);
    }
    let bytes = name.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            decoded.push((high << 4) | low);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    Cow::Owned(String::from_utf8_lossy(&decoded).into_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayHttpRequestFingerprintError {
    InvalidRequestTarget,
    EmptyBodyDigest,
    InvalidBodyDigestCharacter,
}

impl fmt::Display for GatewayHttpRequestFingerprintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequestTarget
            | Self::EmptyBodyDigest
            | Self::InvalidBodyDigestCharacter => {
                write!(f, "request fingerprint is invalid")
            }
        }
    }
}

impl Error for GatewayHttpRequestFingerprintError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayHttpRequestFingerprintErrorStatus {
    BadRequest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayHttpRequestFingerprintErrorResponsePlan {
    pub status: GatewayHttpRequestFingerprintErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_gateway_http_request_fingerprint_error_response(
    _error: &GatewayHttpRequestFingerprintError,
) -> GatewayHttpRequestFingerprintErrorResponsePlan {
    GatewayHttpRequestFingerprintErrorResponsePlan {
        status: GatewayHttpRequestFingerprintErrorStatus::BadRequest,
        code: "request_fingerprint_invalid",
        message: "request fingerprint is invalid",
    }
}

pub fn control_plane_request_fingerprint(
    request: &GatewayHttpRequestMeta,
    body_digest: impl AsRef<str>,
) -> Result<String, GatewayHttpRequestFingerprintError> {
    let target = CanonicalRequestTarget::parse(request.path.as_str())
        .map_err(|_| GatewayHttpRequestFingerprintError::InvalidRequestTarget)?;
    let path = target.path();
    let body_digest = body_digest.as_ref().trim();
    if body_digest.is_empty() {
        return Err(GatewayHttpRequestFingerprintError::EmptyBodyDigest);
    }
    if body_digest
        .chars()
        .any(|character| !character.is_ascii_graphic())
    {
        return Err(GatewayHttpRequestFingerprintError::InvalidBodyDigestCharacter);
    }
    Ok(format!(
        "http:{method}:path:{path}:body:{body_digest}",
        method = method_fingerprint_token(request.method),
    ))
}

fn method_fingerprint_token(method: GatewayHttpMethod) -> &'static str {
    match method {
        GatewayHttpMethod::Get => "get",
        GatewayHttpMethod::Post => "post",
        GatewayHttpMethod::Patch => "patch",
        GatewayHttpMethod::Delete => "delete",
        GatewayHttpMethod::Options => "options",
        GatewayHttpMethod::Other => "other",
    }
}
