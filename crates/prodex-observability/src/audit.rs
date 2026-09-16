use prodex_domain::{TelemetryAttribute, TelemetryAttributeError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditOperation {
    Emit,
    Persist,
    Export,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditResult {
    Success,
    Failure,
    Dropped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditQueryLifecycleOperation {
    PlanQuery,
    PageQuery,
    PlanExport,
    SerializeExport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditQueryLifecycleResult {
    Planned,
    PageReturned,
    Empty,
    Denied,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditQueryLifecycleMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditChainOperation {
    Append,
    VerifyLink,
    VerifyRange,
    ExportProof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditChainResult {
    Success,
    Conflict,
    DigestInvalid,
    GapDetected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditChainMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditRetentionPurgeOperation {
    SelectCandidates,
    ApplyLegalHold,
    DeleteBatch,
    VerifyChain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditRetentionPurgeResult {
    Success,
    Protected,
    Empty,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditRetentionPurgeMetricPlan {
    pub metric_name: &'static str,
    pub increment: u64,
    pub operation_label: TelemetryAttribute,
    pub result_label: TelemetryAttribute,
}

pub fn plan_audit_metric(
    operation: AuditOperation,
    result: AuditResult,
) -> Result<AuditMetricPlan, TelemetryAttributeError> {
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(37, "audit_operation"),
        audit_operation_label(operation),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(40, "audit_result"),
        audit_result_label(result),
    )?;
    Ok(AuditMetricPlan {
        metric_name: crate::planning_support::metric_name(25, 0, "prodex_audit_events_total"),
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_query_lifecycle_metric(
    operation: AuditQueryLifecycleOperation,
    result: AuditQueryLifecycleResult,
) -> Result<AuditQueryLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(38, "audit_query_operation"),
        audit_query_lifecycle_operation_label(operation),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(39, "audit_query_result"),
        audit_query_lifecycle_result_label(result),
    )?;
    Ok(AuditQueryLifecycleMetricPlan {
        metric_name: crate::planning_support::metric_name(
            26,
            0,
            "prodex_audit_query_lifecycle_events_total",
        ),
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_chain_metric(
    operation: AuditChainOperation,
    result: AuditChainResult,
) -> Result<AuditChainMetricPlan, TelemetryAttributeError> {
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(35, "audit_chain_operation"),
        audit_chain_operation_label(operation),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(36, "audit_chain_result"),
        audit_chain_result_label(result),
    )?;
    Ok(AuditChainMetricPlan {
        metric_name: crate::planning_support::metric_name(24, 0, "prodex_audit_chain_events_total"),
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_retention_purge_metric(
    operation: AuditRetentionPurgeOperation,
    result: AuditRetentionPurgeResult,
) -> Result<AuditRetentionPurgeMetricPlan, TelemetryAttributeError> {
    let operation_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(41, "audit_retention_operation"),
        audit_retention_purge_operation_label(operation),
    )?;
    let result_label = crate::planning_support::validated_metric_label(
        crate::planning_support::label_key(42, "audit_retention_result"),
        audit_retention_purge_result_label(result),
    )?;
    Ok(AuditRetentionPurgeMetricPlan {
        metric_name: crate::planning_support::metric_name(
            27,
            0,
            "prodex_audit_retention_purge_events_total",
        ),
        increment: 1,
        operation_label,
        result_label,
    })
}

fn audit_operation_label(operation: AuditOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(48, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            AuditOperation::Emit => "emit",
            AuditOperation::Persist => "persist",
            AuditOperation::Export => "export",
        })
        .to_string()
    }
}

fn audit_result_label(result: AuditResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(51, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            AuditResult::Success => "success",
            AuditResult::Failure => "failure",
            AuditResult::Dropped => "dropped",
        })
        .to_string()
    }
}

fn audit_query_lifecycle_operation_label(operation: AuditQueryLifecycleOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(49, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            AuditQueryLifecycleOperation::PlanQuery => "plan_query",
            AuditQueryLifecycleOperation::PageQuery => "page_query",
            AuditQueryLifecycleOperation::PlanExport => "plan_export",
            AuditQueryLifecycleOperation::SerializeExport => "serialize_export",
        })
        .to_string()
    }
}

fn audit_query_lifecycle_result_label(result: AuditQueryLifecycleResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(50, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            AuditQueryLifecycleResult::Planned => "planned",
            AuditQueryLifecycleResult::PageReturned => "page_returned",
            AuditQueryLifecycleResult::Empty => "empty",
            AuditQueryLifecycleResult::Denied => "denied",
            AuditQueryLifecycleResult::Failed => "failed",
        })
        .to_string()
    }
}

fn audit_chain_operation_label(operation: AuditChainOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(46, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            AuditChainOperation::Append => "append",
            AuditChainOperation::VerifyLink => "verify_link",
            AuditChainOperation::VerifyRange => "verify_range",
            AuditChainOperation::ExportProof => "export_proof",
        })
        .to_string()
    }
}

fn audit_chain_result_label(result: AuditChainResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(47, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            AuditChainResult::Success => "success",
            AuditChainResult::Conflict => "conflict",
            AuditChainResult::DigestInvalid => "digest_invalid",
            AuditChainResult::GapDetected => "gap_detected",
            AuditChainResult::Failed => "failed",
        })
        .to_string()
    }
}

fn audit_retention_purge_operation_label(operation: AuditRetentionPurgeOperation) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(52, operation as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match operation {
            AuditRetentionPurgeOperation::SelectCandidates => "select_candidates",
            AuditRetentionPurgeOperation::ApplyLegalHold => "apply_legal_hold",
            AuditRetentionPurgeOperation::DeleteBatch => "delete_batch",
            AuditRetentionPurgeOperation::VerifyChain => "verify_chain",
        })
        .to_string()
    }
}

fn audit_retention_purge_result_label(result: AuditRetentionPurgeResult) -> String {
    #[cfg(feature = "mojo")]
    {
        prodex_mojo_core::observability::label(53, result as i64)
            .expect("Mojo observability label planner returned invalid output")
    }
    #[cfg(not(feature = "mojo"))]
    {
        (match result {
            AuditRetentionPurgeResult::Success => "success",
            AuditRetentionPurgeResult::Protected => "protected",
            AuditRetentionPurgeResult::Empty => "empty",
            AuditRetentionPurgeResult::Failed => "failed",
        })
        .to_string()
    }
}
