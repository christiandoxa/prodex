use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::event::{AuditEvent, AuditReasonCode, AuditTimestamp, AuditTimestampError};
use super::query::{
    AuditQueryCursor, AuditQueryScope, AuditQueryScopeError, AuditSortOrder,
    compare_audit_event_to_cursor_position, compare_audit_events,
};
use crate::api::{CursorError, Page};
use crate::{AuditEventId, TenantContext, TenantId};

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
            Self::Retention(AuditRetentionPlanError::Timestamp(
                AuditTimestampError::TooFarFuture { .. },
            ))
            | Self::Hold(AuditRetentionHoldError::Timestamp(AuditTimestampError::TooFarFuture {
                ..
            })) => {
                write!(f, "audit timestamp is too far in the future")
            }
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
