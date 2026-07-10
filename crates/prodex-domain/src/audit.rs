use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::api::{Cursor, CursorError, Page};
use crate::{AuditEventId, IdParseError, Principal, PrincipalId, TenantContext, TenantId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Denied,
    Failed,
}

impl AuditOutcome {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, AuditOutcomeError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(AuditOutcomeError::Empty);
        }
        match value {
            "success" => Ok(Self::Success),
            "denied" => Ok(Self::Denied),
            "failed" => Ok(Self::Failed),
            _ => Err(AuditOutcomeError::Unknown),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Denied => "denied",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditOutcomeError {
    Empty,
    Unknown,
}

impl fmt::Debug for AuditOutcomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

impl fmt::Display for AuditOutcomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit outcome is invalid")
    }
}

impl Error for AuditOutcomeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditExportFormat {
    Jsonl,
    Csv,
}

impl AuditExportFormat {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, AuditExportFormatError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(AuditExportFormatError::Empty);
        }
        match value {
            "jsonl" => Ok(Self::Jsonl),
            "csv" => Ok(Self::Csv),
            _ => Err(AuditExportFormatError::Unknown),
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Jsonl => "application/x-ndjson",
            Self::Csv => "text/csv",
        }
    }

    pub fn file_extension(self) -> &'static str {
        match self {
            Self::Jsonl => "jsonl",
            Self::Csv => "csv",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditExportFormatError {
    Empty,
    Unknown,
}

impl fmt::Debug for AuditExportFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

impl fmt::Display for AuditExportFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit export format is invalid")
    }
}

impl Error for AuditExportFormatError {}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditExportPlan {
    pub query: AuditQueryPlan,
    pub format: AuditExportFormat,
}

impl fmt::Debug for AuditExportPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditExportPlan")
            .field("query", &self.query)
            .field("format", &self.format)
            .finish()
    }
}

impl AuditExportPlan {
    pub fn new(query: AuditQueryPlan, format: AuditExportFormat) -> Self {
        Self { query, format }
    }

    pub fn content_type(self) -> &'static str {
        self.format.content_type()
    }

    pub fn file_extension(self) -> &'static str {
        self.format.file_extension()
    }

    pub fn matches_event(self, event: &AuditEvent) -> Result<bool, AuditQueryPlanError> {
        self.query.matches_event(event)
    }

    pub fn select_events<'a, I>(self, events: I) -> Result<Vec<&'a AuditEvent>, AuditQueryPlanError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        self.query.select_events(events)
    }

    pub fn select_events_after<'a, I>(
        self,
        events: I,
        after: Option<AuditQueryCursor>,
    ) -> Result<Vec<&'a AuditEvent>, AuditQueryPlanError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        self.query.select_events_after(events, after)
    }

    pub fn page_events<'a, I>(
        self,
        events: I,
        after: Option<AuditQueryCursor>,
    ) -> Result<Page<&'a AuditEvent>, AuditQueryPageError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        self.query.page_events(events, after)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditQueryCursor {
    pub occurred_at: AuditTimestamp,
    pub event_id: AuditEventId,
    pub sort_order: AuditSortOrder,
}

impl fmt::Debug for AuditQueryCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditQueryCursor")
            .field("occurred_at", &"<redacted>")
            .field("event_id", &"<redacted>")
            .field("sort_order", &self.sort_order)
            .finish()
    }
}

impl AuditQueryCursor {
    const PREFIX: &'static str = "audit";
    const VERSION: &'static str = "v1";

    pub fn from_event(
        event: &AuditEvent,
        sort_order: AuditSortOrder,
    ) -> Result<Self, AuditTimestampError> {
        Ok(Self {
            occurred_at: AuditTimestamp::new(event.occurred_at_unix_ms)?,
            event_id: event.id,
            sort_order,
        })
    }

    pub fn to_cursor(self) -> Result<Cursor, CursorError> {
        Cursor::new(format!(
            "{}:{}:{}:{}",
            Self::PREFIX,
            Self::VERSION,
            self.sort_order.as_str(),
            self.position_token()
        ))
    }

    pub fn try_parse(value: impl Into<String>) -> Result<Self, AuditQueryCursorError> {
        let cursor = Cursor::new(value.into()).map_err(AuditQueryCursorError::Cursor)?;
        Self::parse(&cursor)
    }

    pub fn parse(cursor: &Cursor) -> Result<Self, AuditQueryCursorError> {
        let mut parts = cursor.as_str().split(':');
        let prefix = parts.next().ok_or(AuditQueryCursorError::Malformed)?;
        let version = parts.next().ok_or(AuditQueryCursorError::Malformed)?;
        let sort_order = parts.next().ok_or(AuditQueryCursorError::Malformed)?;
        let timestamp = parts.next().ok_or(AuditQueryCursorError::Malformed)?;
        let event_id = parts.next().ok_or(AuditQueryCursorError::Malformed)?;
        if parts.next().is_some() {
            return Err(AuditQueryCursorError::Malformed);
        }
        if prefix != Self::PREFIX {
            return Err(AuditQueryCursorError::Malformed);
        }
        if version != Self::VERSION {
            return Err(AuditQueryCursorError::UnsupportedVersion);
        }

        let sort_order =
            AuditSortOrder::parse(sort_order).map_err(AuditQueryCursorError::SortOrder)?;
        let timestamp = timestamp
            .parse::<u64>()
            .map_err(|_| AuditQueryCursorError::Malformed)
            .and_then(|timestamp| {
                AuditTimestamp::new(timestamp).map_err(AuditQueryCursorError::Timestamp)
            })?;
        let event_id = AuditEventId::from_str(event_id).map_err(AuditQueryCursorError::EventId)?;

        Ok(Self {
            occurred_at: timestamp,
            event_id,
            sort_order,
        })
    }

    fn position_token(self) -> String {
        format!("{}:{}", self.occurred_at.unix_ms(), self.event_id)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditQueryCursorError {
    Cursor(CursorError),
    Malformed,
    UnsupportedVersion,
    SortOrder(AuditSortOrderError),
    Timestamp(AuditTimestampError),
    EventId(IdParseError),
}

impl fmt::Debug for AuditQueryCursorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cursor(error) => f.debug_tuple("Cursor").field(error).finish(),
            Self::Malformed => f.write_str("Malformed"),
            Self::UnsupportedVersion => f.write_str("UnsupportedVersion"),
            Self::SortOrder(error) => f.debug_tuple("SortOrder").field(error).finish(),
            Self::Timestamp(error) => f.debug_tuple("Timestamp").field(error).finish(),
            Self::EventId(error) => f.debug_tuple("EventId").field(error).finish(),
        }
    }
}

impl fmt::Display for AuditQueryCursorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cursor(_)
            | Self::Malformed
            | Self::UnsupportedVersion
            | Self::SortOrder(_)
            | Self::Timestamp(_)
            | Self::EventId(_) => write!(f, "audit query cursor is invalid"),
        }
    }
}

impl Error for AuditQueryCursorError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSortOrder {
    OccurredAtAsc,
    OccurredAtDesc,
}

impl AuditSortOrder {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, AuditSortOrderError> {
        let value = value.as_ref();
        if value.is_empty() {
            return Err(AuditSortOrderError::Empty);
        }
        match value {
            "occurred_at_asc" => Ok(Self::OccurredAtAsc),
            "occurred_at_desc" => Ok(Self::OccurredAtDesc),
            _ => Err(AuditSortOrderError::Unknown),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::OccurredAtAsc => "occurred_at_asc",
            Self::OccurredAtDesc => "occurred_at_desc",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditSortOrderError {
    Empty,
    Unknown,
}

impl fmt::Debug for AuditSortOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::Unknown => f.write_str("Unknown"),
        }
    }
}

impl fmt::Display for AuditSortOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit sort order is invalid")
    }
}

impl Error for AuditSortOrderError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditAction(String);

impl fmt::Debug for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuditAction").field(&"<redacted>").finish()
    }
}

impl AuditAction {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn try_new(value: impl Into<String>) -> Result<Self, AuditActionError> {
        let action = Self(value.into());
        action.validate()?;
        Ok(action)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn validate(&self) -> Result<(), AuditActionError> {
        validate_audit_action(&self.0)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditActionError {
    Empty,
    TooLong { length: usize },
    EmptySegment,
    InvalidCharacter,
}

impl fmt::Debug for AuditActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::EmptySegment => f.write_str("EmptySegment"),
            Self::InvalidCharacter => f.write_str("InvalidCharacter"),
        }
    }
}

impl fmt::Display for AuditActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit action is invalid")
    }
}

impl Error for AuditActionError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditResource {
    pub kind: String,
    pub id: Option<String>,
    pub tenant_id: Option<TenantId>,
}

impl fmt::Debug for AuditResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditResource")
            .field("kind", &self.kind)
            .field("id", &self.id.as_deref().map(|_| "<redacted>"))
            .field("tenant_id", &self.tenant_id.map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditResourceId(String);

impl AuditResourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, AuditResourceIdError> {
        let value = value.into();
        validate_audit_resource_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AuditResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuditResourceId")
            .field(&"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditResourceIdError {
    Empty,
    TooLong { length: usize },
    InvalidCharacter,
}

impl fmt::Debug for AuditResourceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::InvalidCharacter => f.write_str("InvalidCharacter"),
        }
    }
}

impl fmt::Display for AuditResourceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit resource id is invalid")
    }
}

impl Error for AuditResourceIdError {}

impl AuditResource {
    pub fn new(
        kind: impl Into<String>,
        id: Option<impl Into<String>>,
        tenant_id: Option<TenantId>,
    ) -> Self {
        let kind = kind.into();
        Self {
            kind: if validate_audit_resource_kind(&kind).is_ok() {
                kind
            } else {
                "audit.invalid_resource".to_string()
            },
            id: id
                .map(Into::into)
                .filter(|id| validate_audit_resource_id(id).is_ok()),
            tenant_id,
        }
    }

    pub fn try_new(
        kind: impl Into<String>,
        id: Option<impl Into<String>>,
        tenant_id: Option<TenantId>,
    ) -> Result<Self, AuditResourceKindError> {
        let kind = kind.into();
        validate_audit_resource_kind(&kind)?;
        Ok(Self::new(kind, id, tenant_id))
    }

    pub fn new_with_resource_id(
        kind: impl Into<String>,
        id: Option<AuditResourceId>,
        tenant_id: Option<TenantId>,
    ) -> Result<Self, AuditResourceKindError> {
        let kind = kind.into();
        validate_audit_resource_kind(&kind)?;
        Ok(Self::new(kind, id.map(|id| id.0), tenant_id))
    }

    pub fn validate_kind(&self) -> Result<(), AuditResourceKindError> {
        validate_audit_resource_kind(&self.kind)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditResourceKindError {
    Empty,
    TooLong { length: usize },
    EmptySegment,
    InvalidCharacter,
}

impl fmt::Debug for AuditResourceKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::EmptySegment => f.write_str("EmptySegment"),
            Self::InvalidCharacter => f.write_str("InvalidCharacter"),
        }
    }
}

impl fmt::Display for AuditResourceKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit resource kind is invalid")
    }
}

impl Error for AuditResourceKindError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: AuditEventId,
    pub occurred_at_unix_ms: u64,
    pub tenant_id: TenantId,
    pub principal_id: PrincipalId,
    pub action: AuditAction,
    pub resource: AuditResource,
    pub outcome: AuditOutcome,
    pub reason_code: Option<String>,
}

impl fmt::Debug for AuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditEvent")
            .field("id", &"<redacted>")
            .field("occurred_at_unix_ms", &"<redacted>")
            .field("tenant_id", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("action", &self.action)
            .field("resource", &self.resource)
            .field("outcome", &self.outcome)
            .field(
                "reason_code",
                &self.reason_code.as_deref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditQueryScope {
    pub tenant_id: TenantId,
}

impl fmt::Debug for AuditQueryScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditQueryScope")
            .field("tenant_id", &"<redacted>")
            .finish()
    }
}

impl AuditQueryScope {
    pub fn tenant(tenant: TenantContext) -> Self {
        Self {
            tenant_id: tenant.tenant_id,
        }
    }

    pub fn authorize_event(self, event: &AuditEvent) -> Result<(), AuditQueryScopeError> {
        if event.tenant_id != self.tenant_id {
            return Err(AuditQueryScopeError::CrossTenantEvent);
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditQueryScopeError {
    CrossTenantEvent,
}

impl fmt::Debug for AuditQueryScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CrossTenantEvent => f.write_str("CrossTenantEvent"),
        }
    }
}

impl fmt::Display for AuditQueryScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit query scope is invalid")
    }
}

impl Error for AuditQueryScopeError {}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditQueryPlan {
    pub scope: AuditQueryScope,
    pub time_range: AuditTimeRange,
    pub page_limit: AuditPageLimit,
    pub sort_order: AuditSortOrder,
}

impl fmt::Debug for AuditQueryPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditQueryPlan")
            .field("scope", &self.scope)
            .field("time_range", &"<redacted>")
            .field("page_limit", &"<redacted>")
            .field("sort_order", &self.sort_order)
            .finish()
    }
}

impl AuditQueryPlan {
    pub fn new(
        scope: AuditQueryScope,
        time_range: AuditTimeRange,
        page_limit: AuditPageLimit,
        sort_order: AuditSortOrder,
    ) -> Self {
        Self {
            scope,
            time_range,
            page_limit,
            sort_order,
        }
    }

    pub fn matches_event(self, event: &AuditEvent) -> Result<bool, AuditQueryPlanError> {
        self.scope
            .authorize_event(event)
            .map_err(AuditQueryPlanError::Scope)?;
        let occurred_at = AuditTimestamp::new(event.occurred_at_unix_ms)
            .map_err(AuditQueryPlanError::Timestamp)?;
        Ok(self.time_range.contains(occurred_at))
    }

    pub fn select_events<'a, I>(self, events: I) -> Result<Vec<&'a AuditEvent>, AuditQueryPlanError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        self.select_events_after(events, None)
    }

    pub fn select_events_after<'a, I>(
        self,
        events: I,
        after: Option<AuditQueryCursor>,
    ) -> Result<Vec<&'a AuditEvent>, AuditQueryPlanError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        if let Some(after) = after
            && after.sort_order != self.sort_order
        {
            return Err(AuditQueryPlanError::CursorSortOrderMismatch);
        }

        let mut selected = self.collect_events_after(events, after)?;
        selected.truncate(self.page_limit.get().into());
        Ok(selected)
    }

    pub fn page_events<'a, I>(
        self,
        events: I,
        after: Option<AuditQueryCursor>,
    ) -> Result<Page<&'a AuditEvent>, AuditQueryPageError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        let mut selected = self
            .collect_events_after(events, after)
            .map_err(AuditQueryPageError::Query)?;
        let limit = usize::from(self.page_limit.get());
        let has_next = selected.len() > limit;
        if has_next {
            selected.truncate(limit);
        }
        let next_cursor = if has_next {
            selected
                .last()
                .map(|event| {
                    AuditQueryCursor::from_event(event, self.sort_order)
                        .map_err(AuditQueryPageError::Timestamp)?
                        .to_cursor()
                        .map_err(AuditQueryPageError::Cursor)
                })
                .transpose()?
        } else {
            None
        };

        Ok(Page::new(selected, next_cursor))
    }

    fn collect_events_after<'a, I>(
        self,
        events: I,
        after: Option<AuditQueryCursor>,
    ) -> Result<Vec<&'a AuditEvent>, AuditQueryPlanError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        if let Some(after) = after
            && after.sort_order != self.sort_order
        {
            return Err(AuditQueryPlanError::CursorSortOrderMismatch);
        }

        let mut selected = Vec::new();
        for event in events {
            let after_cursor = after.is_none_or(|after| {
                compare_audit_event_to_cursor_position(event, after) == Ordering::Greater
            });
            if after_cursor && self.matches_event(event)? {
                selected.push(event);
            }
        }

        selected.sort_by(|left, right| compare_audit_events(left, right, self.sort_order));
        Ok(selected)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditQueryPlanError {
    Scope(AuditQueryScopeError),
    Timestamp(AuditTimestampError),
    CursorSortOrderMismatch,
}

impl fmt::Debug for AuditQueryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => f.debug_tuple("Scope").field(error).finish(),
            Self::Timestamp(error) => f.debug_tuple("Timestamp").field(error).finish(),
            Self::CursorSortOrderMismatch => f.write_str("CursorSortOrderMismatch"),
        }
    }
}

impl fmt::Display for AuditQueryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => error.fmt(f),
            Self::Timestamp(_) => write!(f, "audit query cursor is invalid"),
            Self::CursorSortOrderMismatch => write!(f, "audit query cursor is invalid"),
        }
    }
}

impl Error for AuditQueryPlanError {}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditQueryPageError {
    Query(AuditQueryPlanError),
    Timestamp(AuditTimestampError),
    Cursor(CursorError),
}

impl fmt::Debug for AuditQueryPageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Query(error) => f.debug_tuple("Query").field(error).finish(),
            Self::Timestamp(error) => f.debug_tuple("Timestamp").field(error).finish(),
            Self::Cursor(error) => f.debug_tuple("Cursor").field(error).finish(),
        }
    }
}

impl fmt::Display for AuditQueryPageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Query(error) => error.fmt(f),
            Self::Timestamp(_) => write!(f, "audit query cursor is invalid"),
            Self::Cursor(error) => error.fmt(f),
        }
    }
}

impl Error for AuditQueryPageError {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuditTimestamp {
    unix_ms: u64,
}

impl fmt::Debug for AuditTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditTimestamp")
            .field("unix_ms", &"<redacted>")
            .finish()
    }
}

impl AuditTimestamp {
    pub fn new(unix_ms: u64) -> Result<Self, AuditTimestampError> {
        validate_audit_timestamp(unix_ms)?;
        Ok(Self { unix_ms })
    }

    pub fn unix_ms(self) -> u64 {
        self.unix_ms
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditTimestampError {
    Zero,
    BeforeUnixMilliseconds,
    TooFarFuture { unix_ms: u64 },
}

impl fmt::Debug for AuditTimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => f.write_str("Zero"),
            Self::BeforeUnixMilliseconds => f.write_str("BeforeUnixMilliseconds"),
            Self::TooFarFuture { .. } => f
                .debug_struct("TooFarFuture")
                .field("unix_ms", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditTimestampError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit timestamp is invalid")
    }
}

impl Error for AuditTimestampError {}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditTimeRange {
    pub start: Option<AuditTimestamp>,
    pub end: Option<AuditTimestamp>,
}

impl fmt::Debug for AuditTimeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditTimeRange")
            .field("start", &self.start.map(|_| "<redacted>"))
            .field("end", &self.end.map(|_| "<redacted>"))
            .finish()
    }
}

impl AuditTimeRange {
    pub fn new(
        start: Option<AuditTimestamp>,
        end: Option<AuditTimestamp>,
    ) -> Result<Self, AuditTimeRangeError> {
        if let (Some(start), Some(end)) = (start, end)
            && start.unix_ms() > end.unix_ms()
        {
            return Err(AuditTimeRangeError::StartAfterEnd);
        }
        Ok(Self { start, end })
    }

    pub fn contains(self, timestamp: AuditTimestamp) -> bool {
        self.start
            .is_none_or(|start| timestamp.unix_ms() >= start.unix_ms())
            && self
                .end
                .is_none_or(|end| timestamp.unix_ms() <= end.unix_ms())
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditTimeRangeError {
    StartAfterEnd,
}

impl fmt::Debug for AuditTimeRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartAfterEnd => f.write_str("StartAfterEnd"),
        }
    }
}

impl fmt::Display for AuditTimeRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartAfterEnd => write!(f, "audit time range is invalid"),
        }
    }
}

impl Error for AuditTimeRangeError {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuditPageLimit {
    value: u16,
}

impl fmt::Debug for AuditPageLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditPageLimit")
            .field("value", &"<redacted>")
            .finish()
    }
}

impl AuditPageLimit {
    pub const DEFAULT: u16 = 100;
    pub const MAX: u16 = 1_000;

    pub fn new(value: Option<u16>) -> Result<Self, AuditPageLimitError> {
        let value = value.unwrap_or(Self::DEFAULT);
        if value == 0 {
            return Err(AuditPageLimitError::Zero);
        }
        if value > Self::MAX {
            return Err(AuditPageLimitError::TooLarge { value });
        }
        Ok(Self { value })
    }

    pub fn get(self) -> u16 {
        self.value
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditPageLimitError {
    Zero,
    TooLarge { value: u16 },
}

impl fmt::Debug for AuditPageLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => f.write_str("Zero"),
            Self::TooLarge { .. } => f
                .debug_struct("TooLarge")
                .field("value", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditPageLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "audit page limit is invalid")
    }
}

impl Error for AuditPageLimitError {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuditRetentionPolicy {
    days: u16,
}

impl fmt::Debug for AuditRetentionPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionPolicy")
            .field("days", &"<redacted>")
            .finish()
    }
}

impl AuditRetentionPolicy {
    pub const MIN_DAYS: u16 = 30;
    pub const DEFAULT_DAYS: u16 = 365;
    pub const MAX_DAYS: u16 = 3_650;

    pub fn new(days: Option<u16>) -> Result<Self, AuditRetentionPolicyError> {
        let days = days.unwrap_or(Self::DEFAULT_DAYS);
        if days < Self::MIN_DAYS {
            return Err(AuditRetentionPolicyError::TooShort { days });
        }
        if days > Self::MAX_DAYS {
            return Err(AuditRetentionPolicyError::TooLong { days });
        }
        Ok(Self { days })
    }

    pub fn days(self) -> u16 {
        self.days
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionPolicyError {
    TooShort { days: u16 },
    TooLong { days: u16 },
}

impl fmt::Debug for AuditRetentionPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { .. } => f
                .debug_struct("TooShort")
                .field("days", &"<redacted>")
                .finish(),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("days", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditRetentionPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { .. } | Self::TooLong { .. } => {
                write!(f, "audit retention policy is invalid")
            }
        }
    }
}

impl Error for AuditRetentionPolicyError {}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuditRetentionBatchLimit {
    value: u16,
}

impl fmt::Debug for AuditRetentionBatchLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionBatchLimit")
            .field("value", &"<redacted>")
            .finish()
    }
}

impl AuditRetentionBatchLimit {
    pub const DEFAULT: u16 = 100;
    pub const MAX: u16 = 1_000;

    pub fn new(value: Option<u16>) -> Result<Self, AuditRetentionBatchLimitError> {
        let value = value.unwrap_or(Self::DEFAULT);
        if value == 0 {
            return Err(AuditRetentionBatchLimitError::Zero);
        }
        if value > Self::MAX {
            return Err(AuditRetentionBatchLimitError::TooLarge { value });
        }
        Ok(Self { value })
    }

    pub fn get(self) -> u16 {
        self.value
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionBatchLimitError {
    Zero,
    TooLarge { value: u16 },
}

impl fmt::Debug for AuditRetentionBatchLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => f.write_str("Zero"),
            Self::TooLarge { .. } => f
                .debug_struct("TooLarge")
                .field("value", &"<redacted>")
                .finish(),
        }
    }
}

impl fmt::Display for AuditRetentionBatchLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(f, "audit retention batch limit is invalid"),
            Self::TooLarge { .. } => write!(f, "audit retention batch limit is too large"),
        }
    }
}

impl Error for AuditRetentionBatchLimitError {}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRetentionPlan {
    pub scope: AuditQueryScope,
    pub policy: AuditRetentionPolicy,
    pub now: AuditTimestamp,
}

impl fmt::Debug for AuditRetentionPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionPlan")
            .field("scope", &self.scope)
            .field("policy", &"<redacted>")
            .field("now", &"<redacted>")
            .finish()
    }
}

impl AuditRetentionPlan {
    const MILLIS_PER_DAY: u64 = 86_400_000;

    pub fn new(scope: AuditQueryScope, policy: AuditRetentionPolicy, now: AuditTimestamp) -> Self {
        Self { scope, policy, now }
    }

    pub fn cutoff(self) -> AuditTimestamp {
        let retention_ms = u64::from(self.policy.days()) * Self::MILLIS_PER_DAY;
        let cutoff = self
            .now
            .unix_ms()
            .saturating_sub(retention_ms)
            .max(AuditEvent::MIN_UNIX_MS);
        AuditTimestamp { unix_ms: cutoff }
    }

    pub fn is_event_expired(self, event: &AuditEvent) -> Result<bool, AuditRetentionPlanError> {
        self.scope
            .authorize_event(event)
            .map_err(AuditRetentionPlanError::Scope)?;
        let occurred_at = AuditTimestamp::new(event.occurred_at_unix_ms)
            .map_err(AuditRetentionPlanError::Timestamp)?;
        Ok(occurred_at.unix_ms() < self.cutoff().unix_ms())
    }

    pub fn purge_candidates<'a, I>(
        self,
        events: I,
        batch_limit: AuditRetentionBatchLimit,
    ) -> Result<Vec<&'a AuditEvent>, AuditRetentionPlanError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        let mut selected = Vec::new();
        for event in events {
            if self.is_event_expired(event)? {
                selected.push(event);
            }
        }
        selected.sort_by(|left, right| {
            compare_audit_events(left, right, AuditSortOrder::OccurredAtAsc)
        });
        selected.truncate(batch_limit.get().into());
        Ok(selected)
    }

    pub fn purgeable_candidates<'a, 'h, I, H>(
        self,
        events: I,
        holds: H,
        batch_limit: AuditRetentionBatchLimit,
    ) -> Result<Vec<&'a AuditEvent>, AuditRetentionDecisionError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
        H: IntoIterator<Item = &'h AuditRetentionHold>,
    {
        let holds = holds.into_iter().collect::<Vec<_>>();
        let mut selected = Vec::new();
        for event in events {
            if self.purge_decision(event, holds.iter().copied())? == AuditRetentionDecision::Purge {
                selected.push(event);
            }
        }
        selected.sort_by(|left, right| {
            compare_audit_events(left, right, AuditSortOrder::OccurredAtAsc)
        });
        selected.truncate(batch_limit.get().into());
        Ok(selected)
    }

    pub fn purge_candidate_page<'a, I>(
        self,
        events: I,
        batch_limit: AuditRetentionBatchLimit,
        after: Option<AuditQueryCursor>,
    ) -> Result<Page<&'a AuditEvent>, AuditRetentionPageError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
    {
        if let Some(after) = after
            && after.sort_order != AuditSortOrder::OccurredAtAsc
        {
            return Err(AuditRetentionPageError::CursorSortOrderMismatch);
        }

        let mut selected = Vec::new();
        for event in events {
            let after_cursor = after.is_none_or(|after| {
                compare_audit_event_to_cursor_position(event, after) == Ordering::Greater
            });
            if after_cursor
                && self
                    .is_event_expired(event)
                    .map_err(AuditRetentionPageError::Retention)?
            {
                selected.push(event);
            }
        }
        selected.sort_by(|left, right| {
            compare_audit_events(left, right, AuditSortOrder::OccurredAtAsc)
        });

        let limit = usize::from(batch_limit.get());
        let has_next = selected.len() > limit;
        if has_next {
            selected.truncate(limit);
        }
        let next_cursor = if has_next {
            selected
                .last()
                .map(|event| {
                    AuditQueryCursor::from_event(event, AuditSortOrder::OccurredAtAsc)
                        .map_err(AuditRetentionPageError::Timestamp)?
                        .to_cursor()
                        .map_err(AuditRetentionPageError::Cursor)
                })
                .transpose()?
        } else {
            None
        };

        Ok(Page::new(selected, next_cursor))
    }

    pub fn purgeable_candidate_page<'a, 'h, I, H>(
        self,
        events: I,
        holds: H,
        batch_limit: AuditRetentionBatchLimit,
        after: Option<AuditQueryCursor>,
    ) -> Result<Page<&'a AuditEvent>, AuditRetentionPageError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
        H: IntoIterator<Item = &'h AuditRetentionHold>,
    {
        if let Some(after) = after
            && after.sort_order != AuditSortOrder::OccurredAtAsc
        {
            return Err(AuditRetentionPageError::CursorSortOrderMismatch);
        }

        let holds = holds.into_iter().collect::<Vec<_>>();
        let mut selected = Vec::new();
        for event in events {
            let after_cursor = after.is_none_or(|after| {
                compare_audit_event_to_cursor_position(event, after) == Ordering::Greater
            });
            if after_cursor
                && self
                    .purge_decision(event, holds.iter().copied())
                    .map_err(AuditRetentionPageError::Decision)?
                    == AuditRetentionDecision::Purge
            {
                selected.push(event);
            }
        }
        selected.sort_by(|left, right| {
            compare_audit_events(left, right, AuditSortOrder::OccurredAtAsc)
        });

        let limit = usize::from(batch_limit.get());
        let has_next = selected.len() > limit;
        if has_next {
            selected.truncate(limit);
        }
        let next_cursor = if has_next {
            selected
                .last()
                .map(|event| {
                    AuditQueryCursor::from_event(event, AuditSortOrder::OccurredAtAsc)
                        .map_err(AuditRetentionPageError::Timestamp)?
                        .to_cursor()
                        .map_err(AuditRetentionPageError::Cursor)
                })
                .transpose()?
        } else {
            None
        };

        Ok(Page::new(selected, next_cursor))
    }

    pub fn purgeable_candidate_key_page<'a, 'h, I, H>(
        self,
        events: I,
        holds: H,
        batch_limit: AuditRetentionBatchLimit,
        after: Option<AuditQueryCursor>,
    ) -> Result<Page<AuditRetentionPurgeKey>, AuditRetentionPageError>
    where
        I: IntoIterator<Item = &'a AuditEvent>,
        H: IntoIterator<Item = &'h AuditRetentionHold>,
    {
        let page = self.purgeable_candidate_page(events, holds, batch_limit, after)?;
        let mut keys = Vec::with_capacity(page.items.len());
        for event in page.items {
            keys.push(
                AuditRetentionPurgeKey::from_event(self.scope, event)
                    .map_err(AuditRetentionPageError::Retention)?,
            );
        }
        Ok(Page::new(keys, page.next_cursor))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionPlanError {
    Scope(AuditQueryScopeError),
    Timestamp(AuditTimestampError),
}

impl fmt::Debug for AuditRetentionPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => f.debug_tuple("Scope").field(error).finish(),
            Self::Timestamp(error) => f.debug_tuple("Timestamp").field(error).finish(),
        }
    }
}

impl fmt::Display for AuditRetentionPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => error.fmt(f),
            Self::Timestamp(AuditTimestampError::TooFarFuture { .. }) => {
                write!(f, "audit timestamp is too far in the future")
            }
            Self::Timestamp(error) => error.fmt(f),
        }
    }
}

impl Error for AuditRetentionPlanError {}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionPageError {
    Retention(AuditRetentionPlanError),
    Decision(AuditRetentionDecisionError),
    CursorSortOrderMismatch,
    Timestamp(AuditTimestampError),
    Cursor(CursorError),
}

impl fmt::Debug for AuditRetentionPageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retention(error) => f.debug_tuple("Retention").field(error).finish(),
            Self::Decision(error) => f.debug_tuple("Decision").field(error).finish(),
            Self::CursorSortOrderMismatch => f.write_str("CursorSortOrderMismatch"),
            Self::Timestamp(error) => f.debug_tuple("Timestamp").field(error).finish(),
            Self::Cursor(error) => f.debug_tuple("Cursor").field(error).finish(),
        }
    }
}

impl fmt::Display for AuditRetentionPageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retention(error) => error.fmt(f),
            Self::Decision(error) => error.fmt(f),
            Self::CursorSortOrderMismatch => write!(f, "audit query cursor is invalid"),
            Self::Timestamp(AuditTimestampError::TooFarFuture { .. }) => {
                write!(f, "audit timestamp is too far in the future")
            }
            Self::Timestamp(error) => error.fmt(f),
            Self::Cursor(error) => error.fmt(f),
        }
    }
}

impl Error for AuditRetentionPageError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRetentionHold {
    pub tenant_id: TenantId,
    pub event_id: AuditEventId,
    pub reason_code: AuditReasonCode,
    pub expires_at: Option<AuditTimestamp>,
}

impl fmt::Debug for AuditRetentionHold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionHold")
            .field("tenant_id", &"<redacted>")
            .field("event_id", &"<redacted>")
            .field("reason_code", &"<redacted>")
            .field("expires_at", &self.expires_at.map(|_| "<redacted>"))
            .finish()
    }
}

impl AuditRetentionHold {
    pub fn new(
        tenant: TenantContext,
        event_id: AuditEventId,
        reason_code: AuditReasonCode,
        expires_at: Option<AuditTimestamp>,
    ) -> Self {
        Self {
            tenant_id: tenant.tenant_id,
            event_id,
            reason_code,
            expires_at,
        }
    }

    pub fn is_active(&self, now: AuditTimestamp) -> bool {
        self.expires_at
            .is_none_or(|expires_at| now.unix_ms() <= expires_at.unix_ms())
    }

    pub fn protects_event(
        &self,
        event: &AuditEvent,
        now: AuditTimestamp,
    ) -> Result<bool, AuditRetentionHoldError> {
        if event.tenant_id != self.tenant_id {
            return Err(AuditRetentionHoldError::Scope(
                AuditQueryScopeError::CrossTenantEvent,
            ));
        }
        AuditTimestamp::new(event.occurred_at_unix_ms)
            .map_err(AuditRetentionHoldError::Timestamp)?;
        Ok(self.event_id == event.id && self.is_active(now))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionHoldError {
    Scope(AuditQueryScopeError),
    Timestamp(AuditTimestampError),
}

impl fmt::Debug for AuditRetentionHoldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => f.debug_tuple("Scope").field(error).finish(),
            Self::Timestamp(error) => f.debug_tuple("Timestamp").field(error).finish(),
        }
    }
}

impl fmt::Display for AuditRetentionHoldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => error.fmt(f),
            Self::Timestamp(AuditTimestampError::TooFarFuture { .. }) => {
                write!(f, "audit timestamp is too far in the future")
            }
            Self::Timestamp(error) => error.fmt(f),
        }
    }
}

impl Error for AuditRetentionHoldError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditRetentionDecision {
    Retain,
    ProtectedByHold,
    Purge,
}

impl AuditRetentionPlan {
    pub fn purge_decision<'a, I>(
        self,
        event: &AuditEvent,
        holds: I,
    ) -> Result<AuditRetentionDecision, AuditRetentionDecisionError>
    where
        I: IntoIterator<Item = &'a AuditRetentionHold>,
    {
        if !self
            .is_event_expired(event)
            .map_err(AuditRetentionDecisionError::Retention)?
        {
            return Ok(AuditRetentionDecision::Retain);
        }

        for hold in holds {
            if hold
                .protects_event(event, self.now)
                .map_err(AuditRetentionDecisionError::Hold)?
            {
                return Ok(AuditRetentionDecision::ProtectedByHold);
            }
        }

        Ok(AuditRetentionDecision::Purge)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionDecisionError {
    Retention(AuditRetentionPlanError),
    Hold(AuditRetentionHoldError),
}

impl fmt::Debug for AuditRetentionDecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retention(error) => f.debug_tuple("Retention").field(error).finish(),
            Self::Hold(error) => f.debug_tuple("Hold").field(error).finish(),
        }
    }
}

impl fmt::Display for AuditRetentionDecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retention(error) => error.fmt(f),
            Self::Hold(error) => error.fmt(f),
        }
    }
}

impl Error for AuditRetentionDecisionError {}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRetentionPurgeKey {
    pub tenant_id: TenantId,
    pub event_id: AuditEventId,
}

impl fmt::Debug for AuditRetentionPurgeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionPurgeKey")
            .field("tenant_id", &"<redacted>")
            .field("event_id", &"<redacted>")
            .finish()
    }
}

impl AuditRetentionPurgeKey {
    pub fn from_event(
        scope: AuditQueryScope,
        event: &AuditEvent,
    ) -> Result<Self, AuditRetentionPlanError> {
        scope
            .authorize_event(event)
            .map_err(AuditRetentionPlanError::Scope)?;
        AuditTimestamp::new(event.occurred_at_unix_ms)
            .map_err(AuditRetentionPlanError::Timestamp)?;

        Ok(Self {
            tenant_id: event.tenant_id,
            event_id: event.immutable_key(),
        })
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRetentionPurgeBatch {
    pub tenant_id: TenantId,
    pub keys: Vec<AuditRetentionPurgeKey>,
}

impl fmt::Debug for AuditRetentionPurgeBatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditRetentionPurgeBatch")
            .field("tenant_id", &"<redacted>")
            .field("key_count", &self.keys.len())
            .finish()
    }
}

impl AuditRetentionPurgeBatch {
    pub fn new<I>(
        scope: AuditQueryScope,
        keys: I,
        batch_limit: AuditRetentionBatchLimit,
    ) -> Result<Self, AuditRetentionPurgeBatchError>
    where
        I: IntoIterator<Item = AuditRetentionPurgeKey>,
    {
        let keys = keys.into_iter().collect::<Vec<_>>();
        let limit = usize::from(batch_limit.get());
        if keys.len() > limit {
            return Err(AuditRetentionPurgeBatchError::TooManyKeys {
                count: keys.len(),
                limit,
            });
        }
        for key in &keys {
            if key.tenant_id != scope.tenant_id {
                return Err(AuditRetentionPurgeBatchError::CrossTenantKey);
            }
        }

        Ok(Self {
            tenant_id: scope.tenant_id,
            keys,
        })
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn event_ids(&self) -> impl Iterator<Item = AuditEventId> + '_ {
        self.keys.iter().map(|key| key.event_id)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditRetentionPurgeBatchError {
    TooManyKeys { count: usize, limit: usize },
    CrossTenantKey,
}

impl fmt::Debug for AuditRetentionPurgeBatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyKeys { .. } => f
                .debug_struct("TooManyKeys")
                .field("count", &"<redacted>")
                .field("limit", &"<redacted>")
                .finish(),
            Self::CrossTenantKey => f.write_str("CrossTenantKey"),
        }
    }
}

impl fmt::Display for AuditRetentionPurgeBatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyKeys { .. } | Self::CrossTenantKey => {
                write!(f, "audit retention purge batch is invalid")
            }
        }
    }
}

impl Error for AuditRetentionPurgeBatchError {}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReasonCode(String);

impl fmt::Debug for AuditReasonCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuditReasonCode")
            .field(&"<redacted>")
            .finish()
    }
}

impl AuditReasonCode {
    pub fn new(value: impl Into<String>) -> Result<Self, AuditReasonCodeError> {
        let value = value.into();
        validate_audit_reason_code(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditReasonCodeError {
    Empty,
    TooLong { length: usize },
    EmptySegment,
    InvalidCharacter,
}

impl fmt::Debug for AuditReasonCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::EmptySegment => f.write_str("EmptySegment"),
            Self::InvalidCharacter => f.write_str("InvalidCharacter"),
        }
    }
}

impl fmt::Display for AuditReasonCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty | Self::TooLong { .. } | Self::EmptySegment | Self::InvalidCharacter => {
                write!(f, "audit reason code is invalid")
            }
        }
    }
}

impl Error for AuditReasonCodeError {}

impl AuditEvent {
    pub const MIN_UNIX_MS: u64 = 1_000;
    pub const MAX_UNIX_MS: u64 = 4_102_444_800_000;

    pub fn new(
        occurred_at_unix_ms: u64,
        tenant: TenantContext,
        principal: &Principal,
        action: AuditAction,
        resource: AuditResource,
        outcome: AuditOutcome,
        reason_code: Option<impl Into<String>>,
    ) -> Self {
        Self {
            id: AuditEventId::new(),
            occurred_at_unix_ms,
            tenant_id: tenant.tenant_id,
            principal_id: principal.id,
            action: if action.validate().is_ok() {
                action
            } else {
                AuditAction("audit.invalid_action".to_string())
            },
            resource,
            outcome,
            reason_code: reason_code
                .map(Into::into)
                .filter(|reason_code| validate_audit_reason_code(reason_code).is_ok()),
        }
    }

    pub fn new_at(
        occurred_at: AuditTimestamp,
        tenant: TenantContext,
        principal: &Principal,
        action: AuditAction,
        resource: AuditResource,
        outcome: AuditOutcome,
        reason_code: Option<impl Into<String>>,
    ) -> Self {
        Self::new(
            occurred_at.unix_ms(),
            tenant,
            principal,
            action,
            resource,
            outcome,
            reason_code,
        )
    }

    pub fn new_with_reason_code(
        occurred_at_unix_ms: u64,
        tenant: TenantContext,
        principal: &Principal,
        action: AuditAction,
        resource: AuditResource,
        outcome: AuditOutcome,
        reason_code: Option<AuditReasonCode>,
    ) -> Self {
        Self::new(
            occurred_at_unix_ms,
            tenant,
            principal,
            action,
            resource,
            outcome,
            reason_code.map(|reason_code| reason_code.0),
        )
    }

    pub fn immutable_key(&self) -> AuditEventId {
        self.id
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditDigest(String);

impl AuditDigest {
    pub fn new(value: impl Into<String>) -> Result<Self, AuditDigestError> {
        let value = value.into();
        validate_audit_digest(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for AuditDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AuditDigest").field(&"<redacted>").finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuditDigestError {
    Empty,
    TooLong { length: usize },
    InvalidCharacter,
}

impl fmt::Debug for AuditDigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::TooLong { .. } => f
                .debug_struct("TooLong")
                .field("length", &"<redacted>")
                .finish(),
            Self::InvalidCharacter => f.write_str("InvalidCharacter"),
        }
    }
}

impl fmt::Display for AuditDigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty | Self::TooLong { .. } | Self::InvalidCharacter => {
                write!(f, "audit digest is invalid")
            }
        }
    }
}

impl Error for AuditDigestError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditErrorStatus {
    InvalidRequest,
    Forbidden,
    Conflict,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditErrorResponsePlan {
    pub status: AuditErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for AuditErrorResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditErrorResponsePlan")
            .field("status", &self.status)
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

pub fn plan_audit_digest_error_response(error: &AuditDigestError) -> AuditErrorResponsePlan {
    let code = match error {
        AuditDigestError::Empty => "audit_digest_required",
        AuditDigestError::TooLong { .. } | AuditDigestError::InvalidCharacter => {
            "audit_digest_invalid"
        }
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit digest is invalid",
    }
}

pub fn plan_audit_outcome_error_response(error: &AuditOutcomeError) -> AuditErrorResponsePlan {
    let code = match error {
        AuditOutcomeError::Empty => "audit_outcome_required",
        AuditOutcomeError::Unknown => "audit_outcome_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit outcome is invalid",
    }
}

pub fn plan_audit_export_format_error_response(
    error: &AuditExportFormatError,
) -> AuditErrorResponsePlan {
    let code = match error {
        AuditExportFormatError::Empty => "audit_export_format_required",
        AuditExportFormatError::Unknown => "audit_export_format_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit export format is invalid",
    }
}

pub fn plan_audit_sort_order_error_response(error: &AuditSortOrderError) -> AuditErrorResponsePlan {
    let code = match error {
        AuditSortOrderError::Empty => "audit_sort_order_required",
        AuditSortOrderError::Unknown => "audit_sort_order_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit sort order is invalid",
    }
}

pub fn plan_audit_timestamp_error_response(error: &AuditTimestampError) -> AuditErrorResponsePlan {
    let code = match error {
        AuditTimestampError::Zero => "audit_timestamp_required",
        AuditTimestampError::BeforeUnixMilliseconds | AuditTimestampError::TooFarFuture { .. } => {
            "audit_timestamp_invalid"
        }
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit timestamp is invalid",
    }
}

pub fn plan_audit_time_range_error_response(error: &AuditTimeRangeError) -> AuditErrorResponsePlan {
    let code = match error {
        AuditTimeRangeError::StartAfterEnd => "audit_time_range_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit time range is invalid",
    }
}

pub fn plan_audit_page_limit_error_response(error: &AuditPageLimitError) -> AuditErrorResponsePlan {
    let code = match error {
        AuditPageLimitError::Zero => "audit_page_limit_required",
        AuditPageLimitError::TooLarge { .. } => "audit_page_limit_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit page limit is invalid",
    }
}

pub fn plan_audit_retention_policy_error_response(
    error: &AuditRetentionPolicyError,
) -> AuditErrorResponsePlan {
    let code = match error {
        AuditRetentionPolicyError::TooShort { .. } | AuditRetentionPolicyError::TooLong { .. } => {
            "audit_retention_policy_invalid"
        }
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit retention policy is invalid",
    }
}

pub fn plan_audit_retention_batch_limit_error_response(
    error: &AuditRetentionBatchLimitError,
) -> AuditErrorResponsePlan {
    let code = match error {
        AuditRetentionBatchLimitError::Zero => "audit_retention_batch_limit_required",
        AuditRetentionBatchLimitError::TooLarge { .. } => "audit_retention_batch_limit_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit retention batch limit is invalid",
    }
}

pub fn plan_audit_retention_purge_batch_error_response(
    error: &AuditRetentionPurgeBatchError,
) -> AuditErrorResponsePlan {
    match error {
        AuditRetentionPurgeBatchError::TooManyKeys { .. } => AuditErrorResponsePlan {
            status: AuditErrorStatus::InvalidRequest,
            code: "audit_retention_purge_batch_too_large",
            message: "audit retention purge batch is invalid",
        },
        AuditRetentionPurgeBatchError::CrossTenantKey => AuditErrorResponsePlan {
            status: AuditErrorStatus::Forbidden,
            code: "audit_retention_purge_batch_scope_denied",
            message: "audit retention purge batch is invalid",
        },
    }
}

pub fn plan_audit_retention_plan_error_response(
    error: &AuditRetentionPlanError,
) -> AuditErrorResponsePlan {
    match error {
        AuditRetentionPlanError::Scope(error) => plan_audit_query_scope_error_response(error),
        AuditRetentionPlanError::Timestamp(error) => plan_audit_timestamp_error_response(error),
    }
}

pub fn plan_audit_retention_page_error_response(
    error: &AuditRetentionPageError,
) -> AuditErrorResponsePlan {
    match error {
        AuditRetentionPageError::Retention(error) => {
            plan_audit_retention_plan_error_response(error)
        }
        AuditRetentionPageError::Decision(error) => {
            plan_audit_retention_decision_error_response(error)
        }
        AuditRetentionPageError::CursorSortOrderMismatch => plan_audit_query_cursor_error_response(
            &AuditQueryCursorError::SortOrder(AuditSortOrderError::Unknown),
        ),
        AuditRetentionPageError::Timestamp(error) => plan_audit_timestamp_error_response(error),
        AuditRetentionPageError::Cursor(error) => {
            plan_audit_query_cursor_error_response(&AuditQueryCursorError::Cursor(error.clone()))
        }
    }
}

pub fn plan_audit_retention_hold_error_response(
    error: &AuditRetentionHoldError,
) -> AuditErrorResponsePlan {
    match error {
        AuditRetentionHoldError::Scope(error) => plan_audit_query_scope_error_response(error),
        AuditRetentionHoldError::Timestamp(error) => plan_audit_timestamp_error_response(error),
    }
}

pub fn plan_audit_retention_decision_error_response(
    error: &AuditRetentionDecisionError,
) -> AuditErrorResponsePlan {
    match error {
        AuditRetentionDecisionError::Retention(error) => {
            plan_audit_retention_plan_error_response(error)
        }
        AuditRetentionDecisionError::Hold(error) => plan_audit_retention_hold_error_response(error),
    }
}

pub fn plan_audit_query_scope_error_response(
    error: &AuditQueryScopeError,
) -> AuditErrorResponsePlan {
    let code = match error {
        AuditQueryScopeError::CrossTenantEvent => "audit_query_scope_denied",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::Forbidden,
        code,
        message: "audit query scope is invalid",
    }
}

pub fn plan_audit_query_plan_error_response(error: &AuditQueryPlanError) -> AuditErrorResponsePlan {
    match error {
        AuditQueryPlanError::Scope(error) => plan_audit_query_scope_error_response(error),
        AuditQueryPlanError::Timestamp(error) => plan_audit_timestamp_error_response(error),
        AuditQueryPlanError::CursorSortOrderMismatch => plan_audit_query_cursor_error_response(
            &AuditQueryCursorError::SortOrder(AuditSortOrderError::Unknown),
        ),
    }
}

pub fn plan_audit_query_cursor_error_response(
    error: &AuditQueryCursorError,
) -> AuditErrorResponsePlan {
    let code = match error {
        AuditQueryCursorError::Cursor(_)
        | AuditQueryCursorError::Malformed
        | AuditQueryCursorError::UnsupportedVersion
        | AuditQueryCursorError::SortOrder(_)
        | AuditQueryCursorError::Timestamp(_)
        | AuditQueryCursorError::EventId(_) => "audit_query_cursor_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit query cursor is invalid",
    }
}

pub fn plan_audit_query_page_error_response(error: &AuditQueryPageError) -> AuditErrorResponsePlan {
    match error {
        AuditQueryPageError::Query(error) => plan_audit_query_plan_error_response(error),
        AuditQueryPageError::Timestamp(error) => plan_audit_timestamp_error_response(error),
        AuditQueryPageError::Cursor(error) => {
            plan_audit_query_cursor_error_response(&AuditQueryCursorError::Cursor(error.clone()))
        }
    }
}

pub fn plan_audit_action_error_response(error: &AuditActionError) -> AuditErrorResponsePlan {
    let code = match error {
        AuditActionError::Empty => "audit_action_required",
        AuditActionError::TooLong { .. }
        | AuditActionError::EmptySegment
        | AuditActionError::InvalidCharacter => "audit_action_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit action is invalid",
    }
}

pub fn plan_audit_resource_kind_error_response(
    error: &AuditResourceKindError,
) -> AuditErrorResponsePlan {
    let code = match error {
        AuditResourceKindError::Empty => "audit_resource_kind_required",
        AuditResourceKindError::TooLong { .. }
        | AuditResourceKindError::EmptySegment
        | AuditResourceKindError::InvalidCharacter => "audit_resource_kind_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit resource kind is invalid",
    }
}

pub fn plan_audit_resource_id_error_response(
    error: &AuditResourceIdError,
) -> AuditErrorResponsePlan {
    let code = match error {
        AuditResourceIdError::Empty => "audit_resource_id_required",
        AuditResourceIdError::TooLong { .. } | AuditResourceIdError::InvalidCharacter => {
            "audit_resource_id_invalid"
        }
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit resource id is invalid",
    }
}

pub fn plan_audit_reason_code_error_response(
    error: &AuditReasonCodeError,
) -> AuditErrorResponsePlan {
    let code = match error {
        AuditReasonCodeError::Empty => "audit_reason_code_required",
        AuditReasonCodeError::TooLong { .. }
        | AuditReasonCodeError::EmptySegment
        | AuditReasonCodeError::InvalidCharacter => "audit_reason_code_invalid",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::InvalidRequest,
        code,
        message: "audit reason code is invalid",
    }
}

pub fn plan_audit_chain_error_response(error: &AuditChainError) -> AuditErrorResponsePlan {
    let code = match error {
        AuditChainError::PreviousDigestMismatch => "audit_chain_conflict",
    };

    AuditErrorResponsePlan {
        status: AuditErrorStatus::Conflict,
        code,
        message: "audit chain verification failed",
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEnvelope {
    pub event: AuditEvent,
    pub previous_digest: Option<AuditDigest>,
    pub event_digest: AuditDigest,
}

impl fmt::Debug for AuditEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditEnvelope")
            .field("event", &self.event)
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}

impl AuditEnvelope {
    pub fn new(
        event: AuditEvent,
        previous_digest: Option<AuditDigest>,
        event_digest: AuditDigest,
    ) -> Self {
        Self {
            event,
            previous_digest,
            event_digest,
        }
    }

    pub fn verify_chain_link(
        &self,
        expected_previous_digest: Option<&AuditDigest>,
    ) -> Result<(), AuditChainError> {
        if self.previous_digest.as_ref() != expected_previous_digest {
            return Err(AuditChainError::PreviousDigestMismatch);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuditChainError {
    PreviousDigestMismatch,
}

impl fmt::Debug for AuditChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreviousDigestMismatch => f.write_str("PreviousDigestMismatch"),
        }
    }
}

fn compare_audit_events(
    left: &AuditEvent,
    right: &AuditEvent,
    sort_order: AuditSortOrder,
) -> Ordering {
    let time_order = left.occurred_at_unix_ms.cmp(&right.occurred_at_unix_ms);
    let time_order = match sort_order {
        AuditSortOrder::OccurredAtAsc => time_order,
        AuditSortOrder::OccurredAtDesc => time_order.reverse(),
    };
    time_order.then_with(|| left.id.cmp(&right.id))
}

fn compare_audit_event_to_cursor_position(
    event: &AuditEvent,
    cursor: AuditQueryCursor,
) -> Ordering {
    let time_order = event.occurred_at_unix_ms.cmp(&cursor.occurred_at.unix_ms());
    let time_order = match cursor.sort_order {
        AuditSortOrder::OccurredAtAsc => time_order,
        AuditSortOrder::OccurredAtDesc => time_order.reverse(),
    };
    time_order.then_with(|| event.id.cmp(&cursor.event_id))
}

fn validate_audit_action(value: &str) -> Result<(), AuditActionError> {
    if value.is_empty() {
        return Err(AuditActionError::Empty);
    }
    if value.len() > 128 {
        return Err(AuditActionError::TooLong {
            length: value.len(),
        });
    }
    if value.starts_with('.') || value.ends_with('.') || value.contains("..") {
        return Err(AuditActionError::EmptySegment);
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
    {
        return Err(AuditActionError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_resource_kind(value: &str) -> Result<(), AuditResourceKindError> {
    if value.is_empty() {
        return Err(AuditResourceKindError::Empty);
    }
    if value.len() > 96 {
        return Err(AuditResourceKindError::TooLong {
            length: value.len(),
        });
    }
    if value.starts_with('.') || value.ends_with('.') || value.contains("..") {
        return Err(AuditResourceKindError::EmptySegment);
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
    {
        return Err(AuditResourceKindError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_resource_id(value: &str) -> Result<(), AuditResourceIdError> {
    if value.is_empty() {
        return Err(AuditResourceIdError::Empty);
    }
    if value.len() > 256 {
        return Err(AuditResourceIdError::TooLong {
            length: value.len(),
        });
    }
    if !value.chars().all(|ch| {
        ch.is_ascii_lowercase()
            || ch.is_ascii_digit()
            || ch == '.'
            || ch == '_'
            || ch == '-'
            || ch == ':'
    }) {
        return Err(AuditResourceIdError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_reason_code(value: &str) -> Result<(), AuditReasonCodeError> {
    if value.is_empty() {
        return Err(AuditReasonCodeError::Empty);
    }
    if value.len() > 128 {
        return Err(AuditReasonCodeError::TooLong {
            length: value.len(),
        });
    }
    if value.starts_with('.') || value.ends_with('.') || value.contains("..") {
        return Err(AuditReasonCodeError::EmptySegment);
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '.')
    {
        return Err(AuditReasonCodeError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_digest(value: &str) -> Result<(), AuditDigestError> {
    if value.is_empty() {
        return Err(AuditDigestError::Empty);
    }
    if value.len() > 128 {
        return Err(AuditDigestError::TooLong {
            length: value.len(),
        });
    }
    if !value.chars().all(|ch| {
        ch.is_ascii_lowercase()
            || ch.is_ascii_digit()
            || ch == ':'
            || ch == '.'
            || ch == '_'
            || ch == '-'
    }) {
        return Err(AuditDigestError::InvalidCharacter);
    }
    Ok(())
}

fn validate_audit_timestamp(unix_ms: u64) -> Result<(), AuditTimestampError> {
    if unix_ms == 0 {
        return Err(AuditTimestampError::Zero);
    }
    if unix_ms < AuditEvent::MIN_UNIX_MS {
        return Err(AuditTimestampError::BeforeUnixMilliseconds);
    }
    if unix_ms > AuditEvent::MAX_UNIX_MS {
        return Err(AuditTimestampError::TooFarFuture { unix_ms });
    }
    Ok(())
}
