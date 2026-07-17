use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
const RUNTIME_GATEWAY_RECONCILIATION_LIMIT: usize = 4_096;
const RUNTIME_GATEWAY_RECONCILIATION_WORKER_LIMIT: usize = 4;

struct RuntimeGatewayReconciliationJob {
    event: RuntimeProviderGatewaySpendEvent,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

#[derive(Clone)]
pub(super) struct RuntimeGatewayReconciliationQueue {
    slots: Arc<tokio::sync::Semaphore>,
    admissions: Arc<Mutex<BTreeMap<u64, tokio::sync::OwnedSemaphorePermit>>>,
    pending: Arc<Mutex<VecDeque<RuntimeGatewayReconciliationJob>>>,
    workers: Arc<AtomicUsize>,
}

impl RuntimeGatewayReconciliationQueue {
    pub(super) fn new() -> Self {
        Self {
            slots: Arc::new(tokio::sync::Semaphore::new(
                RUNTIME_GATEWAY_RECONCILIATION_LIMIT,
            )),
            admissions: Arc::new(Mutex::new(BTreeMap::new())),
            pending: Arc::new(Mutex::new(VecDeque::new())),
            workers: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(super) fn try_reserve(&self) -> Option<tokio::sync::OwnedSemaphorePermit> {
        Arc::clone(&self.slots).try_acquire_owned().ok()
    }

    pub(super) fn commit(&self, request_id: u64, permit: tokio::sync::OwnedSemaphorePermit) {
        self.admissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(request_id, permit);
    }

    pub(super) fn cancel(&self, request_id: u64) {
        self.admissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&request_id);
    }

    fn enqueue(&self, event: RuntimeProviderGatewaySpendEvent) -> bool {
        let permit = self
            .admissions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&event.request);
        let Some(permit) = permit else {
            return false;
        };
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(RuntimeGatewayReconciliationJob {
                event,
                _permit: permit,
            });
        true
    }

    fn pop(&self) -> Option<RuntimeGatewayReconciliationJob> {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
    }

    fn pending_len(&self) -> usize {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

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
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&event.request);
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
    let request = event.request;
    if !shared.gateway_usage.reconciliation.enqueue(event) {
        crate::runtime_proxy_log(
            &shared.runtime_shared,
            runtime_proxy_structured_log_message(
                "gateway_billing_ledger_reconcile_skipped",
                [
                    runtime_proxy_log_field("request", request.to_string()),
                    runtime_proxy_log_field("backend", shared.gateway_state_store.label()),
                ],
            ),
        );
        return;
    }
    start_runtime_gateway_reconciliation_workers(shared);
}

fn start_runtime_gateway_reconciliation_workers(shared: &RuntimeLocalRewriteProxyShared) {
    let queue = &shared.gateway_usage.reconciliation;
    loop {
        let workers = queue.workers.load(Ordering::Acquire);
        let wanted = queue
            .pending_len()
            .min(RUNTIME_GATEWAY_RECONCILIATION_WORKER_LIMIT);
        if workers >= wanted {
            return;
        }
        if queue
            .workers
            .compare_exchange(workers, workers + 1, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            continue;
        }
        let shared = shared.clone();
        let background_task = RuntimeGatewayBackgroundTaskGuard::new(&shared);
        drop(
            shared
                .runtime_shared
                .async_runtime
                .clone()
                .spawn_blocking(move || {
                    let _background_task = background_task;
                    runtime_gateway_reconciliation_worker_loop(shared);
                }),
        );
    }
}

fn runtime_gateway_reconciliation_worker_loop(shared: RuntimeLocalRewriteProxyShared) {
    while let Some(job) = shared.gateway_usage.reconciliation.pop() {
        runtime_gateway_reconcile_until_persisted(&shared, &job.event);
    }
    shared
        .gateway_usage
        .reconciliation
        .workers
        .fetch_sub(1, Ordering::AcqRel);
    if shared.gateway_usage.reconciliation.pending_len() > 0 {
        start_runtime_gateway_reconciliation_workers(&shared);
    }
}

fn runtime_gateway_reconcile_until_persisted(
    shared: &RuntimeLocalRewriteProxyShared,
    event: &RuntimeProviderGatewaySpendEvent,
) {
    let state_store = &shared.gateway_state_store;
    let reconciliation = runtime_gateway_application_reconciliation_execution(state_store, event);
    let terminal_attempt = reconciliation.retry.attempts().last().unwrap_or_default();
    let mut attempt = 0;
    let mut ledger_reconciled = false;
    let mut storage_error_observed = false;
    let mut retry_logged = false;
    loop {
        if !ledger_reconciled {
            match runtime_gateway_billing_ledger_reconcile_response(state_store, event) {
                Ok(changed) => ledger_reconciled = changed,
                Err(_err) => storage_error_observed = true,
            }
        }
        if ledger_reconciled {
            match runtime_gateway_durable_reconcile_response(
                &shared.runtime_shared,
                state_store,
                shared.gateway_postgres_repository.as_ref(),
                &shared.gateway_usage.durable_reservations,
                event,
            ) {
                Ok(()) => {
                    runtime_gateway_record_reconciliation_audit(
                        &shared.runtime_shared,
                        event,
                        reconciliation,
                        ApplicationUsageReconciliationAuditOutcome::Success,
                    );
                    runtime_gateway_finish_reconciliation(shared, event.request);
                    return;
                }
                Err(_err) => storage_error_observed = true,
            }
        }
        if attempt == terminal_attempt && !retry_logged {
            retry_logged = true;
            let exhaustion = reconciliation.exhausted(storage_error_observed);
            crate::runtime_proxy_log(
                &shared.runtime_shared,
                runtime_proxy_structured_log_message(
                    "gateway_billing_ledger_reconcile_retrying",
                    [
                        runtime_proxy_log_field("request", event.request.to_string()),
                        runtime_proxy_log_field("backend", state_store.label()),
                        runtime_proxy_log_field("error_kind", exhaustion.error_kind()),
                    ],
                ),
            );
        }
        runtime_gateway_reconciliation_retry_sleep(reconciliation, attempt);
        attempt = attempt.saturating_add(1).min(terminal_attempt);
    }
}

fn runtime_gateway_finish_reconciliation(shared: &RuntimeLocalRewriteProxyShared, request: u64) {
    shared
        .gateway_usage
        .request_ids
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&request);
    shared
        .gateway_usage
        .typed_request_ids
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&request);
    shared
        .gateway_usage
        .call_ids
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&request);
    shared
        .gateway_usage
        .ledger_scopes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&request);
    shared
        .gateway_usage
        .durable_reservations
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&request);
}

fn runtime_gateway_reconciliation_retry_sleep(
    reconciliation: ApplicationUsageReconciliationExecutionPlan,
    attempt: u8,
) {
    std::thread::sleep(
        reconciliation
            .retry
            .backoff_after(attempt)
            .unwrap_or(Duration::from_secs(1)),
    );
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
    fn reconciliation_retry_logs_use_stable_redacted_error_kinds() {
        for error_kind in [
            "gateway_reconciliation_retry_exhausted",
            "gateway_ledger_persistence_failed",
        ] {
            let message = runtime_proxy_structured_log_message(
                "gateway_billing_ledger_reconcile_retrying",
                [
                    runtime_proxy_log_field("request", "123"),
                    runtime_proxy_log_field("backend", "file"),
                    runtime_proxy_log_field("error_kind", error_kind),
                ],
            );
            assert!(message.contains("gateway_billing_ledger_reconcile_retrying"));
            assert!(message.contains("backend=file"));
            assert!(message.contains(error_kind));
            assert!(!message.contains("prodex-audit.log"));
            assert!(!message.contains("/tmp/"));
            assert!(!message.contains("permission denied"));
        }
    }

    #[test]
    fn reconciliation_queue_reservations_are_bounded_and_released() {
        let queue = RuntimeGatewayReconciliationQueue::new();
        let mut permits = Vec::new();
        for _ in 0..RUNTIME_GATEWAY_RECONCILIATION_LIMIT {
            permits.push(queue.try_reserve().unwrap());
        }
        assert!(queue.try_reserve().is_none());

        drop(permits.pop());
        assert!(queue.try_reserve().is_some());
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
