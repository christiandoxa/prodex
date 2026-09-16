use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::event::{AuditEvent, AuditTimestamp, AuditTimestampError};
use crate::api::{Cursor, CursorError, Page};
use crate::{AuditEventId, IdParseError, TenantContext, TenantId};

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
pub(super) fn compare_audit_events(
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

pub(super) fn compare_audit_event_to_cursor_position(
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
