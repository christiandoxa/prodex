use std::{error::Error, fmt};

#[cfg(not(feature = "mojo"))]
use prodex_gateway_core::plan_gateway_virtual_key_admission;
use prodex_gateway_core::{
    GatewayVirtualKeyAdmissionError, GatewayVirtualKeyAdmissionPlan,
    GatewayVirtualKeyAdmissionRequest,
};
#[cfg(feature = "mojo")]
use prodex_gateway_core::{
    GatewayVirtualKeyAdmissionKernelInput, plan_gateway_virtual_key_admission_with_kernel,
};

use crate::distributed_rate_limit::{
    ApplicationDistributedRateLimitError, ApplicationDistributedRateLimitPlan,
    ApplicationDistributedRateLimitRequest, plan_application_distributed_rate_limit,
};

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationVirtualKeyAdmissionPlan {
    pub gateway: GatewayVirtualKeyAdmissionPlan,
    pub distributed_rate_limit: Option<ApplicationDistributedRateLimitPlan>,
}

impl fmt::Debug for ApplicationVirtualKeyAdmissionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationVirtualKeyAdmissionPlan")
            .field("gateway", &"<redacted>")
            .field("distributed_rate_limit", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationVirtualKeyAdmissionError {
    Gateway(GatewayVirtualKeyAdmissionError),
    DistributedRateLimit(ApplicationDistributedRateLimitError),
}

impl fmt::Display for ApplicationVirtualKeyAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gateway(error) => error.fmt(formatter),
            Self::DistributedRateLimit(error) => error.fmt(formatter),
        }
    }
}

impl Error for ApplicationVirtualKeyAdmissionError {}

#[cfg(feature = "mojo")]
fn gateway_virtual_key_admission_kernel(
    input: GatewayVirtualKeyAdmissionKernelInput,
) -> Result<(), GatewayVirtualKeyAdmissionError> {
    let decision = prodex_mojo_core::rich::validate_virtual_key_admission(
        prodex_mojo_core::rich::VirtualKeyAdmissionInput {
            durable_budget: input.durable_budget,
            usage_minute_epoch: input.usage_minute_epoch,
            minute_epoch: input.minute_epoch,
            requests_this_minute: input.requests_this_minute,
            tokens_this_minute: input.tokens_this_minute,
            requests_total: input.requests_total,
            spend_microusd: input.spend_microusd,
            reserved_tokens: input.reserved_tokens,
            estimated_cost_microusd: input.estimated_cost_microusd,
            request_budget: input.request_budget,
            budget_microusd: input.budget_microusd,
            rpm_limit: input.rpm_limit,
            tpm_limit: input.tpm_limit,
        },
    )
    .map_err(|_| GatewayVirtualKeyAdmissionError::PolicyStateUnavailable)?;
    match decision {
        prodex_mojo_core::rich::VirtualKeyAdmissionDecision::Allow => Ok(()),
        prodex_mojo_core::rich::VirtualKeyAdmissionDecision::RequestBudgetExceeded => {
            Err(GatewayVirtualKeyAdmissionError::RequestBudgetExceeded)
        }
        prodex_mojo_core::rich::VirtualKeyAdmissionDecision::BudgetExceeded => {
            Err(GatewayVirtualKeyAdmissionError::BudgetExceeded)
        }
        prodex_mojo_core::rich::VirtualKeyAdmissionDecision::RpmLimitExceeded => {
            Err(GatewayVirtualKeyAdmissionError::RpmLimitExceeded)
        }
        prodex_mojo_core::rich::VirtualKeyAdmissionDecision::TpmLimitExceeded => {
            Err(GatewayVirtualKeyAdmissionError::TpmLimitExceeded)
        }
    }
}

pub fn plan_application_virtual_key_admission(
    request: GatewayVirtualKeyAdmissionRequest,
) -> Result<ApplicationVirtualKeyAdmissionPlan, ApplicationVirtualKeyAdmissionError> {
    let gateway = {
        #[cfg(feature = "mojo")]
        {
            plan_gateway_virtual_key_admission_with_kernel(
                request,
                gateway_virtual_key_admission_kernel,
            )
        }
        #[cfg(not(feature = "mojo"))]
        {
            plan_gateway_virtual_key_admission(request)
        }
    }
    .map_err(ApplicationVirtualKeyAdmissionError::Gateway)?;
    let distributed_rate_limit = gateway
        .distributed_rate_limit
        .as_ref()
        .map(|request| {
            plan_application_distributed_rate_limit(ApplicationDistributedRateLimitRequest {
                bucket: request.bucket,
                window_seconds: request.window_seconds,
                max_requests: request.max_requests,
                max_tokens: request.max_tokens,
                increment_tokens: request.increment_tokens,
                now_unix_ms: request.now_unix_ms,
            })
        })
        .transpose()
        .map_err(ApplicationVirtualKeyAdmissionError::DistributedRateLimit)?;
    Ok(ApplicationVirtualKeyAdmissionPlan {
        gateway,
        distributed_rate_limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_domain::{CallId, ReservationId, TenantId, VirtualKeyId};
    use prodex_gateway_core::{
        GatewayVirtualKeyPolicy, GatewayVirtualKeyReservationContext, GatewayVirtualKeyUsage,
    };

    #[test]
    fn application_plans_distributed_rate_limit_from_canonical_admission() {
        let tenant_id = TenantId::new();
        let plan = plan_application_virtual_key_admission(GatewayVirtualKeyAdmissionRequest {
            policy: GatewayVirtualKeyPolicy {
                name: "team-a".to_string(),
                tenant_id: Some(tenant_id.to_string()),
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                allowed_models: Vec::new(),
                budget_microusd: None,
                request_budget: None,
                rpm_limit: Some(10),
                tpm_limit: Some(100),
            },
            usage: GatewayVirtualKeyUsage::default(),
            grouped_usage: Vec::new(),
            model: Some("gpt-5.4".to_string()),
            input_tokens: 5,
            reserved_tokens: 22,
            estimated_cost_microusd: Some(39),
            minute_epoch: 10,
            reservation: Some(GatewayVirtualKeyReservationContext {
                tenant_id,
                virtual_key_id: Some(VirtualKeyId::new()),
                call_id: CallId::new(),
                reservation_id: ReservationId::new(),
                durable_store: None,
                created_at_unix_ms: 1_000,
                ttl_ms: 60_000,
            }),
            distributed_rate_limit: true,
            now_unix_ms: 600_001,
        })
        .unwrap();

        assert!(plan.distributed_rate_limit.is_some());
        assert_eq!(plan.gateway.admission.reserved_tokens, 22);
        assert_eq!(
            plan.gateway
                .reservation
                .as_ref()
                .unwrap()
                .request
                .estimate
                .cost_micros,
            39
        );
    }
}
