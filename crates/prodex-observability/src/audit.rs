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
    let operation_label =
        TelemetryAttribute::metric_label("audit_operation", audit_operation_label(operation));
    let result_label = TelemetryAttribute::metric_label("audit_result", audit_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuditMetricPlan {
        metric_name: "prodex_audit_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_query_lifecycle_metric(
    operation: AuditQueryLifecycleOperation,
    result: AuditQueryLifecycleResult,
) -> Result<AuditQueryLifecycleMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "audit_query_operation",
        audit_query_lifecycle_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "audit_query_result",
        audit_query_lifecycle_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuditQueryLifecycleMetricPlan {
        metric_name: "prodex_audit_query_lifecycle_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_chain_metric(
    operation: AuditChainOperation,
    result: AuditChainResult,
) -> Result<AuditChainMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "audit_chain_operation",
        audit_chain_operation_label(operation),
    );
    let result_label =
        TelemetryAttribute::metric_label("audit_chain_result", audit_chain_result_label(result));
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuditChainMetricPlan {
        metric_name: "prodex_audit_chain_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

pub fn plan_audit_retention_purge_metric(
    operation: AuditRetentionPurgeOperation,
    result: AuditRetentionPurgeResult,
) -> Result<AuditRetentionPurgeMetricPlan, TelemetryAttributeError> {
    let operation_label = TelemetryAttribute::metric_label(
        "audit_retention_operation",
        audit_retention_purge_operation_label(operation),
    );
    let result_label = TelemetryAttribute::metric_label(
        "audit_retention_result",
        audit_retention_purge_result_label(result),
    );
    operation_label.as_metric_label()?;
    result_label.as_metric_label()?;
    Ok(AuditRetentionPurgeMetricPlan {
        metric_name: "prodex_audit_retention_purge_events_total",
        increment: 1,
        operation_label,
        result_label,
    })
}

fn audit_operation_label(operation: AuditOperation) -> &'static str {
    match operation {
        AuditOperation::Emit => "emit",
        AuditOperation::Persist => "persist",
        AuditOperation::Export => "export",
    }
}

fn audit_result_label(result: AuditResult) -> &'static str {
    match result {
        AuditResult::Success => "success",
        AuditResult::Failure => "failure",
        AuditResult::Dropped => "dropped",
    }
}

fn audit_query_lifecycle_operation_label(operation: AuditQueryLifecycleOperation) -> &'static str {
    match operation {
        AuditQueryLifecycleOperation::PlanQuery => "plan_query",
        AuditQueryLifecycleOperation::PageQuery => "page_query",
        AuditQueryLifecycleOperation::PlanExport => "plan_export",
        AuditQueryLifecycleOperation::SerializeExport => "serialize_export",
    }
}

fn audit_query_lifecycle_result_label(result: AuditQueryLifecycleResult) -> &'static str {
    match result {
        AuditQueryLifecycleResult::Planned => "planned",
        AuditQueryLifecycleResult::PageReturned => "page_returned",
        AuditQueryLifecycleResult::Empty => "empty",
        AuditQueryLifecycleResult::Denied => "denied",
        AuditQueryLifecycleResult::Failed => "failed",
    }
}

fn audit_chain_operation_label(operation: AuditChainOperation) -> &'static str {
    match operation {
        AuditChainOperation::Append => "append",
        AuditChainOperation::VerifyLink => "verify_link",
        AuditChainOperation::VerifyRange => "verify_range",
        AuditChainOperation::ExportProof => "export_proof",
    }
}

fn audit_chain_result_label(result: AuditChainResult) -> &'static str {
    match result {
        AuditChainResult::Success => "success",
        AuditChainResult::Conflict => "conflict",
        AuditChainResult::DigestInvalid => "digest_invalid",
        AuditChainResult::GapDetected => "gap_detected",
        AuditChainResult::Failed => "failed",
    }
}

fn audit_retention_purge_operation_label(operation: AuditRetentionPurgeOperation) -> &'static str {
    match operation {
        AuditRetentionPurgeOperation::SelectCandidates => "select_candidates",
        AuditRetentionPurgeOperation::ApplyLegalHold => "apply_legal_hold",
        AuditRetentionPurgeOperation::DeleteBatch => "delete_batch",
        AuditRetentionPurgeOperation::VerifyChain => "verify_chain",
    }
}

fn audit_retention_purge_result_label(result: AuditRetentionPurgeResult) -> &'static str {
    match result {
        AuditRetentionPurgeResult::Success => "success",
        AuditRetentionPurgeResult::Protected => "protected",
        AuditRetentionPurgeResult::Empty => "empty",
        AuditRetentionPurgeResult::Failed => "failed",
    }
}
