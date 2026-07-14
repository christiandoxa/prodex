//! Pure governance orchestration.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    CanonicalRoute, CapabilitySet, Channel, ClassificationDecision, ClassificationError,
    ClassificationRequest, CompiledClassificationRuleSet, CompiledGovernancePolicy,
    CredentialScope, DataClassification, DataPolicyContext, DetectorRevisionId, EnvironmentContext,
    GovernancePolicyError, GovernedAction, InspectionCoverage, InspectionFinding, InspectionLimits,
    InspectionModelError, InspectionReasonCode, InspectionResult, InspectionTag, PolicyDecision,
    PolicyInput, Principal, QuotaContext, RequestRisk, SessionPolicyContext, TenantContext,
    classify_inspection, evaluate_governance_policy,
};

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationInspectionSource {
    pub coverage: InspectionCoverage,
    pub findings: Vec<InspectionFinding>,
    pub masked_findings: Vec<prodex_domain::FindingKind>,
    pub tags: Vec<InspectionTag>,
    pub reason_codes: Vec<InspectionReasonCode>,
}

impl fmt::Debug for ApplicationInspectionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationInspectionSource")
            .field("coverage", &self.coverage)
            .field("finding_count", &self.findings.len())
            .field("masked_finding_count", &self.masked_findings.len())
            .field("tag_count", &self.tags.len())
            .field("reason_code_count", &self.reason_codes.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationInspectionRequest {
    pub sources: Vec<ApplicationInspectionSource>,
    pub default_classification: DataClassification,
    pub trusted_label: Option<DataClassification>,
    pub prior_classification: Option<DataClassification>,
    pub detector_revision: DetectorRevisionId,
    pub limits: InspectionLimits,
}

impl fmt::Debug for ApplicationInspectionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationInspectionRequest")
            .field("source_count", &self.sources.len())
            .field("default_classification", &self.default_classification)
            .field("trusted_label", &self.trusted_label)
            .field("prior_classification", &self.prior_classification)
            .field("detector_revision", &"<redacted>")
            .field("limits", &self.limits)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationInspectionPlan {
    pub result: InspectionResult,
    pub masked_findings: Vec<prodex_domain::FindingKind>,
}

impl fmt::Debug for ApplicationInspectionPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationInspectionPlan")
            .field("result", &self.result)
            .field("masked_finding_count", &self.masked_findings.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationInspectionError {
    TooManyDetectors,
    TooManyMaskedFindingKinds,
    InvalidResult(InspectionModelError),
}

impl fmt::Display for ApplicationInspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "request inspection metadata is invalid")
    }
}

impl Error for ApplicationInspectionError {}

pub fn plan_application_request_inspection(
    request: ApplicationInspectionRequest,
) -> Result<ApplicationInspectionPlan, ApplicationInspectionError> {
    if request.sources.len() > request.limits.max_detectors {
        return Err(ApplicationInspectionError::TooManyDetectors);
    }

    let mut coverage: Option<InspectionCoverage> = None;
    let mut classification = request.default_classification;
    if let Some(label) = request.trusted_label {
        classification = classification.raised_to(label);
    }
    if let Some(prior) = request.prior_classification {
        classification = classification.raised_to(prior);
    }

    let mut findings = Vec::new();
    let mut masked_findings = Vec::new();
    let mut tags = Vec::new();
    let mut reason_codes = Vec::new();
    for source in request.sources {
        if source.masked_findings.len() > prodex_domain::FindingKind::ALL.len() {
            return Err(ApplicationInspectionError::TooManyMaskedFindingKinds);
        }
        coverage = Some(match coverage {
            Some(current) => current.combine(source.coverage),
            None => source.coverage,
        });
        for kind in source.masked_findings {
            if source.findings.iter().any(|finding| finding.kind() == kind) {
                masked_findings.push(kind);
            }
        }
        for finding in source.findings {
            classification = classification.raised_to(finding.kind().minimum_classification());
            findings.push(finding);
        }
        tags.extend(source.tags);
        reason_codes.extend(source.reason_codes);
    }
    masked_findings.sort();
    masked_findings.dedup();

    let result = InspectionResult::new(
        coverage.unwrap_or(InspectionCoverage::Unsupported),
        classification,
        findings,
        tags,
        reason_codes,
        request.detector_revision,
        request.limits,
    )
    .map_err(ApplicationInspectionError::InvalidResult)?;
    Ok(ApplicationInspectionPlan {
        result,
        masked_findings,
    })
}

#[derive(Clone)]
pub struct ApplicationGovernanceSnapshot {
    pub classification_rules: CompiledClassificationRuleSet,
    pub policy: CompiledGovernancePolicy,
}

impl fmt::Debug for ApplicationGovernanceSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGovernanceSnapshot")
            .field("classification_rules", &self.classification_rules)
            .field("policy", &self.policy)
            .finish()
    }
}

pub struct ApplicationGovernanceRequest<'a> {
    pub inspection: &'a ApplicationInspectionPlan,
    pub trusted_label: Option<DataClassification>,
    pub untrusted_label: Option<DataClassification>,
    pub prior_classification: Option<DataClassification>,
    pub session_floor: DataClassification,
    pub route_floor: DataClassification,
    pub request_risk_floor: DataClassification,
    pub tenant: TenantContext,
    pub principal: &'a Principal,
    pub principal_attributes: &'a prodex_domain::PrincipalPolicyAttributes,
    pub channel: Channel,
    pub credential_scope: CredentialScope,
    pub session: SessionPolicyContext,
    pub action: GovernedAction,
    pub route: &'a CanonicalRoute,
    pub request_risk: RequestRisk,
    pub requested_capabilities: &'a CapabilitySet,
    pub request_attributes: &'a prodex_domain::RequestPolicyAttributes,
    pub quota: QuotaContext,
    pub environment: EnvironmentContext,
}

impl fmt::Debug for ApplicationGovernanceRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGovernanceRequest")
            .field("inspection", &self.inspection)
            .field("trusted_label", &self.trusted_label)
            .field("untrusted_label", &self.untrusted_label)
            .field("prior_classification", &self.prior_classification)
            .field("session_floor", &self.session_floor)
            .field("route_floor", &self.route_floor)
            .field("request_risk_floor", &self.request_risk_floor)
            .field("tenant", &self.tenant)
            .field("principal", &self.principal)
            .field("principal_attributes", &self.principal_attributes)
            .field("channel", &self.channel)
            .field("credential_scope", &self.credential_scope)
            .field("session", &self.session)
            .field("action", &self.action)
            .field("route", &self.route)
            .field("request_risk", &self.request_risk)
            .field("requested_capabilities", &self.requested_capabilities)
            .field("request_attributes", &self.request_attributes)
            .field("quota", &self.quota)
            .field("environment", &self.environment)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ApplicationGovernancePlan {
    pub classification: ClassificationDecision,
    pub policy: PolicyDecision,
}

impl fmt::Debug for ApplicationGovernancePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationGovernancePlan")
            .field("classification", &self.classification)
            .field("policy", &self.policy)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationGovernanceError {
    Classification(ClassificationError),
    Policy(GovernancePolicyError),
}

impl fmt::Display for ApplicationGovernanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("request governance decision failed")
    }
}

impl Error for ApplicationGovernanceError {}

pub fn plan_application_governance(
    snapshot: &ApplicationGovernanceSnapshot,
    request: ApplicationGovernanceRequest<'_>,
) -> Result<ApplicationGovernancePlan, ApplicationGovernanceError> {
    let classification = classify_inspection(
        &snapshot.classification_rules,
        ClassificationRequest {
            inspection: &request.inspection.result,
            trusted_label: request.trusted_label,
            untrusted_label: request.untrusted_label,
            prior_classification: request.prior_classification,
            session_floor: request.session_floor,
            route_floor: request.route_floor,
            request_risk_floor: request.request_risk_floor,
        },
    )
    .map_err(ApplicationGovernanceError::Classification)?;
    let policy = evaluate_governance_policy(
        &snapshot.policy,
        &PolicyInput {
            tenant: request.tenant,
            principal: request.principal,
            principal_attributes: request.principal_attributes,
            channel: request.channel,
            credential_scope: request.credential_scope,
            session: request.session,
            action: request.action,
            route: request.route,
            data: DataPolicyContext {
                classification: classification.classification(),
                inspection_coverage: classification.coverage(),
            },
            request_risk: request.request_risk,
            requested_capabilities: request.requested_capabilities,
            request_attributes: request.request_attributes,
            quota: request.quota,
            environment: request.environment,
        },
    )
    .map_err(ApplicationGovernanceError::Policy)?;
    Ok(ApplicationGovernancePlan {
        classification,
        policy,
    })
}
