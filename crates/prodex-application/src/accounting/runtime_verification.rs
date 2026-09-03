use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimeAccountingVerificationRequest<'a> {
    pub runtime_plan: &'a ApplicationRuntimePlan,
    pub evidence: MultiReplicaAccountingEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRuntimeAccountingVerificationPlan {
    pub verification: MultiReplicaAccountingVerificationPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRuntimeAccountingVerificationError {
    AccountingNotRequired,
    AccountingConcurrency(prodex_storage::MultiReplicaAccountingConcurrencySpecError),
}

impl fmt::Display for ApplicationRuntimeAccountingVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountingNotRequired => write!(
                f,
                "runtime topology did not require multi-replica accounting verification"
            ),
            Self::AccountingConcurrency(err) => err.fmt(f),
        }
    }
}

impl Error for ApplicationRuntimeAccountingVerificationError {}

pub fn plan_application_runtime_accounting_verification_error_response(
    error: &ApplicationRuntimeAccountingVerificationError,
) -> ApplicationRuntimePlanErrorResponsePlan {
    match error {
        ApplicationRuntimeAccountingVerificationError::AccountingNotRequired => {
            ApplicationRuntimePlanErrorResponsePlan {
                status: ApplicationRuntimePlanErrorStatus::InvalidConfiguration,
                code: "runtime_accounting_verification_invalid",
                message: "runtime accounting verification is invalid",
            }
        }
        ApplicationRuntimeAccountingVerificationError::AccountingConcurrency(error) => {
            application_runtime_response_from_storage(
                plan_multi_replica_accounting_error_response(error),
                "runtime_accounting_verification_invalid",
                "runtime accounting verification is invalid",
            )
        }
    }
}

pub fn plan_application_runtime_accounting_verification_required_response()
-> ApplicationRuntimePlanErrorResponsePlan {
    ApplicationRuntimePlanErrorResponsePlan {
        status: ApplicationRuntimePlanErrorStatus::InvalidConfiguration,
        code: "runtime_accounting_verification_invalid",
        message: "runtime accounting verification is invalid",
    }
}

pub fn plan_application_runtime_accounting_verification(
    request: ApplicationRuntimeAccountingVerificationRequest<'_>,
) -> Result<
    ApplicationRuntimeAccountingVerificationPlan,
    ApplicationRuntimeAccountingVerificationError,
> {
    let spec = request
        .runtime_plan
        .accounting_concurrency
        .as_ref()
        .ok_or(ApplicationRuntimeAccountingVerificationError::AccountingNotRequired)?;
    let verification = plan_multi_replica_accounting_verification(spec, request.evidence)
        .map_err(ApplicationRuntimeAccountingVerificationError::AccountingConcurrency)?;
    Ok(ApplicationRuntimeAccountingVerificationPlan { verification })
}
