use std::fmt;

use serde::{Deserialize, Serialize};

use super::event::{
    AuditActionError, AuditChainError, AuditDigestError, AuditOutcomeError, AuditReasonCodeError,
    AuditResourceIdError, AuditResourceKindError, AuditTimestampError,
};
use super::query::{
    AuditExportFormatError, AuditPageLimitError, AuditQueryCursorError, AuditQueryPageError,
    AuditQueryPlanError, AuditQueryScopeError, AuditSortOrderError, AuditTimeRangeError,
};
use super::retention::{
    AuditRetentionBatchLimitError, AuditRetentionDecisionError, AuditRetentionHoldError,
    AuditRetentionPageError, AuditRetentionPlanError, AuditRetentionPolicyError,
    AuditRetentionPurgeBatchError,
};

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
