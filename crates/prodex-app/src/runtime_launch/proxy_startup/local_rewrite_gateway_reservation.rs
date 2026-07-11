use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_keys::RuntimeGatewayDurableReservationError;

pub(super) fn runtime_gateway_postgres_reserve_usage(
    shared: &RuntimeLocalRewriteProxyShared,
    command: prodex_storage::AtomicReservationCommand,
) -> Result<(), RuntimeGatewayDurableReservationError> {
    let repository = shared
        .gateway_postgres_repository
        .as_ref()
        .ok_or(RuntimeGatewayDurableReservationError::Failed)?;
    match shared
        .runtime_shared
        .async_runtime
        .handle()
        .block_on(repository.reserve(command))
        .map_err(|_| RuntimeGatewayDurableReservationError::Failed)?
    {
        prodex_storage_postgres_runtime::ReserveOutcome::Reserved(_)
        | prodex_storage_postgres_runtime::ReserveOutcome::Replayed(_) => Ok(()),
        prodex_storage_postgres_runtime::ReserveOutcome::Rejected(
            prodex_storage_postgres_runtime::ReserveRejection::BudgetLimitExceeded,
        ) => Err(RuntimeGatewayDurableReservationError::Rejected(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::BudgetExceeded,
        )),
        prodex_storage_postgres_runtime::ReserveOutcome::Rejected(
            prodex_storage_postgres_runtime::ReserveRejection::RequestBudgetExceeded,
        ) => Err(RuntimeGatewayDurableReservationError::Rejected(
            runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::RequestBudgetExceeded,
        )),
        prodex_storage_postgres_runtime::ReserveOutcome::Rejected(
            prodex_storage_postgres_runtime::ReserveRejection::Conflict,
        ) => Err(RuntimeGatewayDurableReservationError::Failed),
    }
}
