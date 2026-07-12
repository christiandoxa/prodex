use std::sync::Arc;

use prodex_application::{
    ApplicationUsageReconciliationAuditOutcome, ApplicationUsageReconciliationExecutionPlan,
};

use super::local_rewrite::{RuntimeGatewayBackgroundTaskGuard, RuntimeLocalRewriteProxyShared};
use super::local_rewrite_application_data_plane::runtime_gateway_application_reconciliation_execution;
use super::local_rewrite_gateway_ledger::{
    runtime_gateway_billing_ledger_reconcile_response, runtime_gateway_durable_reconcile_response,
};
use super::local_rewrite_gateway_reconciliation_audit::runtime_gateway_audit_usage_reconciliation;
use super::provider_bridge::RuntimeProviderGatewaySpendEvent;
use super::*;

const RUNTIME_GATEWAY_AUDIT_PERSISTENCE_FAILED_ERROR_KIND: &str =
    "gateway_audit_persistence_failed";

pub(super) fn schedule_runtime_gateway_billing_ledger_reconcile(
    shared: &RuntimeLocalRewriteProxyShared,
    event: RuntimeProviderGatewaySpendEvent,
) {
    if event.phase != "response" {
        return;
    }
    let should_reconcile = shared
        .gateway_usage
        .request_ids
        .lock()
        .map(|request_ids| request_ids.contains(&event.request))
        .unwrap_or(false);
    if !should_reconcile {
        crate::runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_billing_ledger_reconcile_skipped",
                [
                    runtime_proxy_log_field("request", event.request.to_string()),
                    runtime_proxy_log_field("backend", shared.gateway_state_store.label()),
                ],
            ),
        );
        return;
    }
    let state_store = shared.gateway_state_store.clone();
    let reconciliation = runtime_gateway_application_reconciliation_execution(&state_store, &event);
    let runtime_shared = shared.runtime_shared.clone();
    let postgres_repository = shared.gateway_postgres_repository.clone();
    let request_ids = Arc::clone(&shared.gateway_usage.request_ids);
    let typed_request_ids = Arc::clone(&shared.gateway_usage.typed_request_ids);
    let call_ids = Arc::clone(&shared.gateway_usage.call_ids);
    let ledger_scopes = Arc::clone(&shared.gateway_usage.ledger_scopes);
    let durable_reservations = Arc::clone(&shared.gateway_usage.durable_reservations);
    let background_task = RuntimeGatewayBackgroundTaskGuard::new(shared);
    shared.runtime_shared.async_runtime.spawn_blocking(move || {
        let _background_task = background_task;
        let mut storage_error_observed = false;
        for attempt in reconciliation.retry.attempts() {
            match runtime_gateway_billing_ledger_reconcile_response(&state_store, &event) {
                Ok(true) => {
                    match runtime_gateway_durable_reconcile_response(
                        &runtime_shared,
                        &state_store,
                        postgres_repository.as_ref(),
                        &durable_reservations,
                        &event,
                    ) {
                        Ok(()) => {}
                        Err(_err) => {
                            storage_error_observed = true;
                            runtime_gateway_reconciliation_retry_sleep(reconciliation, attempt);
                            continue;
                        }
                    }
                    runtime_gateway_record_reconciliation_audit(
                        &runtime_shared,
                        &event,
                        reconciliation,
                        ApplicationUsageReconciliationAuditOutcome::Success,
                    );
                    if let Ok(mut request_ids) = request_ids.lock() {
                        request_ids.remove(&event.request);
                    }
                    if let Ok(mut typed_request_ids) = typed_request_ids.lock() {
                        typed_request_ids.remove(&event.request);
                    }
                    if let Ok(mut call_ids) = call_ids.lock() {
                        call_ids.remove(&event.request);
                    }
                    if let Ok(mut ledger_scopes) = ledger_scopes.lock() {
                        ledger_scopes.remove(&event.request);
                    }
                    if let Ok(mut durable_reservations) = durable_reservations.lock() {
                        durable_reservations.remove(&event.request);
                    }
                    return;
                }
                Ok(false) => {
                    match runtime_gateway_durable_reconcile_response(
                        &runtime_shared,
                        &state_store,
                        postgres_repository.as_ref(),
                        &durable_reservations,
                        &event,
                    ) {
                        Ok(()) => {}
                        Err(_err) => {
                            storage_error_observed = true;
                            runtime_gateway_reconciliation_retry_sleep(reconciliation, attempt);
                            continue;
                        }
                    }
                    runtime_gateway_reconciliation_retry_sleep(reconciliation, attempt);
                }
                Err(_err) => {
                    storage_error_observed = true;
                    runtime_gateway_reconciliation_retry_sleep(reconciliation, attempt);
                }
            }
        }
        if let Ok(mut request_ids) = request_ids.lock() {
            request_ids.remove(&event.request);
        }
        if let Ok(mut typed_request_ids) = typed_request_ids.lock() {
            typed_request_ids.remove(&event.request);
        }
        if let Ok(mut call_ids) = call_ids.lock() {
            call_ids.remove(&event.request);
        }
        if let Ok(mut ledger_scopes) = ledger_scopes.lock() {
            ledger_scopes.remove(&event.request);
        }
        if let Ok(mut durable_reservations) = durable_reservations.lock() {
            durable_reservations.remove(&event.request);
        }
        runtime_gateway_record_reconciliation_audit(
            &runtime_shared,
            &event,
            reconciliation,
            ApplicationUsageReconciliationAuditOutcome::Failure,
        );
        let exhaustion = reconciliation.exhausted(storage_error_observed);
        crate::runtime_proxy_log(
            &runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_billing_ledger_reconcile_failed",
                [
                    runtime_proxy_log_field("request", event.request.to_string()),
                    runtime_proxy_log_field("backend", state_store.label()),
                    runtime_proxy_log_field("error_kind", exhaustion.error_kind()),
                ],
            ),
        );
    });
}

fn runtime_gateway_reconciliation_retry_sleep(
    reconciliation: ApplicationUsageReconciliationExecutionPlan,
    attempt: u8,
) {
    if let Some(backoff) = reconciliation.retry.backoff_after(attempt) {
        std::thread::sleep(backoff);
    }
}

fn runtime_gateway_record_reconciliation_audit(
    runtime_shared: &RuntimeRotationProxyShared,
    event: &RuntimeProviderGatewaySpendEvent,
    reconciliation: ApplicationUsageReconciliationExecutionPlan,
    outcome: ApplicationUsageReconciliationAuditOutcome,
) {
    let audit = reconciliation.audit(outcome);
    if runtime_gateway_audit_usage_reconciliation(runtime_shared, audit).is_err() {
        runtime_proxy_log(
            runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_usage_reconciliation_audit_failed",
                [
                    runtime_proxy_log_field("request", event.request.to_string()),
                    runtime_proxy_log_field("backend", audit.backend()),
                    runtime_proxy_log_field(
                        "error_kind",
                        RUNTIME_GATEWAY_AUDIT_PERSISTENCE_FAILED_ERROR_KIND,
                    ),
                ],
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciliation_exhaustion_logs_use_stable_redacted_error_kinds() {
        for error_kind in [
            "gateway_reconciliation_retry_exhausted",
            "gateway_ledger_persistence_failed",
        ] {
            let message = runtime_proxy_structured_log_message(
                "gateway_billing_ledger_reconcile_failed",
                [
                    runtime_proxy_log_field("request", "123"),
                    runtime_proxy_log_field("backend", "file"),
                    runtime_proxy_log_field("error_kind", error_kind),
                ],
            );
            assert!(message.contains("gateway_billing_ledger_reconcile_failed"));
            assert!(message.contains("backend=file"));
            assert!(message.contains(error_kind));
            assert!(!message.contains("prodex-audit.log"));
            assert!(!message.contains("/tmp/"));
            assert!(!message.contains("permission denied"));
        }
    }

    #[test]
    fn reconciliation_audit_failure_log_is_stable_and_redacted() {
        let message = runtime_proxy_structured_log_message(
            "gateway_usage_reconciliation_audit_failed",
            [
                runtime_proxy_log_field("request", "123"),
                runtime_proxy_log_field("backend", "file"),
                runtime_proxy_log_field(
                    "error_kind",
                    RUNTIME_GATEWAY_AUDIT_PERSISTENCE_FAILED_ERROR_KIND,
                ),
            ],
        );

        assert!(message.contains("gateway_usage_reconciliation_audit_failed"));
        assert!(message.contains("backend=file"));
        assert!(message.contains("gateway_audit_persistence_failed"));
        assert!(!message.contains("prodex-audit.log"));
        assert!(!message.contains("/tmp/"));
        assert!(!message.contains("permission denied"));
    }
}
