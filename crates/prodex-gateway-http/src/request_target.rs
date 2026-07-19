use std::{error::Error, fmt, str::FromStr};

pub const MAX_REQUEST_TARGET_BYTES: usize = 8 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct CanonicalRequestTarget {
    raw: String,
    path_len: usize,
}

impl CanonicalRequestTarget {
    pub fn parse(raw: impl Into<String>) -> Result<Self, CanonicalRequestTargetError> {
        let raw = raw.into();
        validate_request_target(&raw)?;
        let path_len = raw.find('?').unwrap_or(raw.len());
        Ok(Self { raw, path_len })
    }

    pub fn path(&self) -> &str {
        &self.raw[..self.path_len]
    }

    pub fn query(&self) -> Option<&str> {
        (self.path_len < self.raw.len()).then(|| &self.raw[self.path_len + 1..])
    }

    pub fn path_and_query(&self) -> &str {
        &self.raw
    }

    pub fn with_path(
        &self,
        replacement_path: impl Into<String>,
    ) -> Result<Self, CanonicalRequestTargetError> {
        let replacement = Self::parse(replacement_path)?;
        if replacement.query().is_some() {
            return Err(CanonicalRequestTargetError::ReplacementPathHasQuery);
        }
        let mut raw = replacement.raw;
        if let Some(query) = self.query() {
            raw.push('?');
            raw.push_str(query);
        }
        Self::parse(raw)
    }
}

impl fmt::Debug for CanonicalRequestTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanonicalRequestTarget")
            .field("path", &"<redacted>")
            .field("has_query", &self.query().is_some())
            .field("length", &self.raw.len())
            .finish()
    }
}

impl FromStr for CanonicalRequestTarget {
    type Err = CanonicalRequestTargetError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalRequestTargetError {
    Empty,
    TooLong,
    NonAscii,
    WhitespaceOrControl,
    AbsoluteOrAuthorityForm,
    Fragment,
    LiteralBackslash,
    RepeatedSlash,
    DotSegment,
    MalformedPercentEncoding,
    EncodedNonAscii,
    EncodedDelimiter,
    ReplacementPathHasQuery,
}

impl fmt::Display for CanonicalRequestTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "request target is invalid")
    }
}

impl Error for CanonicalRequestTargetError {}

fn validate_request_target(raw: &str) -> Result<(), CanonicalRequestTargetError> {
    if raw.is_empty() {
        return Err(CanonicalRequestTargetError::Empty);
    }
    if raw.len() > MAX_REQUEST_TARGET_BYTES {
        return Err(CanonicalRequestTargetError::TooLong);
    }
    if !raw.is_ascii() {
        return Err(CanonicalRequestTargetError::NonAscii);
    }
    if raw.bytes().any(|byte| byte <= b' ' || byte == 0x7f) {
        return Err(CanonicalRequestTargetError::WhitespaceOrControl);
    }
    if !raw.starts_with('/') || raw.starts_with("//") {
        return Err(CanonicalRequestTargetError::AbsoluteOrAuthorityForm);
    }
    if raw.contains('#') {
        return Err(CanonicalRequestTargetError::Fragment);
    }
    let path_len = raw.find('?').unwrap_or(raw.len());
    let path = &raw[..path_len];
    if path.contains('\\') {
        return Err(CanonicalRequestTargetError::LiteralBackslash);
    }
    if path.contains("//") {
        return Err(CanonicalRequestTargetError::RepeatedSlash);
    }
    if path.split('/').any(|segment| matches!(segment, "." | "..")) {
        return Err(CanonicalRequestTargetError::DotSegment);
    }
    validate_percent_encoding(raw, path_len)
}

fn validate_percent_encoding(
    raw: &str,
    path_len: usize,
) -> Result<(), CanonicalRequestTargetError> {
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            index += 1;
            continue;
        }
        let Some(encoded) = bytes.get(index + 1..index + 3) else {
            return Err(CanonicalRequestTargetError::MalformedPercentEncoding);
        };
        let Some(high) = hex_value(encoded[0]) else {
            return Err(CanonicalRequestTargetError::MalformedPercentEncoding);
        };
        let Some(low) = hex_value(encoded[1]) else {
            return Err(CanonicalRequestTargetError::MalformedPercentEncoding);
        };
        let decoded = (high << 4) | low;
        if index < path_len && decoded >= 0x80 {
            return Err(CanonicalRequestTargetError::EncodedNonAscii);
        }
        if index < path_len
            && (decoded <= b' '
                || decoded == 0x7f
                || matches!(decoded, b'/' | b'\\' | b'%' | b'?' | b'#' | b'.'))
        {
            return Err(CanonicalRequestTargetError::EncodedDelimiter);
        }
        index += 3;
    }
    Ok(())
}

pub(super) const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
