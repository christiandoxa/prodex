//! Bounded, deterministic, side-effect-free governed provider selection.

use std::error::Error;
use std::fmt;

use prodex_domain::{
    CapabilitySet, DataClassification, GovernanceObligation, PolicyDecision, PolicyEffect,
    PolicyRevisionId, PolicySelector, ProviderTrustTier, SecretRef, TenantContext,
};
use prodex_provider_core::ProviderId;

pub const ROUTING_SCORE_SCALE: u16 = 10_000;
pub const MAX_GOVERNED_ROUTING_CANDIDATES: usize = 64;
pub const MAX_GOVERNED_ROUTING_FALLBACKS: usize = 8;
pub const MAX_GOVERNED_PROVIDER_REGIONS: usize = 16;
pub const MAX_GOVERNED_HARD_FILTER_REASONS: usize = 17;
pub const GOVERNED_SCORE_COMPONENT_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GovernedHardFilterReason {
    TenantMismatch,
    ProviderDisabled,
    ProviderRevoked,
    CircuitOpen,
    QuotaUnavailable,
    InFlightCapReached,
    CredentialUnavailable,
    ClassificationUnsupported,
    CapabilityMissing,
    ProviderNotAllowed,
    ProviderDenied,
    TrustTierInsufficient,
    RegionUnavailable,
    LocalExecutionRequired,
    RetentionProhibited,
    RetentionLimitExceeded,
    TrainingUseProhibited,
}

impl GovernedHardFilterReason {
    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::TenantMismatch => "routing.hard.tenant_mismatch",
            Self::ProviderDisabled => "routing.hard.provider_disabled",
            Self::ProviderRevoked => "routing.hard.provider_revoked",
            Self::CircuitOpen => "routing.hard.circuit_open",
            Self::QuotaUnavailable => "routing.hard.quota_unavailable",
            Self::InFlightCapReached => "routing.hard.inflight_cap_reached",
            Self::CredentialUnavailable => "routing.hard.credential_unavailable",
            Self::ClassificationUnsupported => "routing.hard.classification_unsupported",
            Self::CapabilityMissing => "routing.hard.capability_missing",
            Self::ProviderNotAllowed => "routing.hard.provider_not_allowed",
            Self::ProviderDenied => "routing.hard.provider_denied",
            Self::TrustTierInsufficient => "routing.hard.trust_tier_insufficient",
            Self::RegionUnavailable => "routing.hard.region_unavailable",
            Self::LocalExecutionRequired => "routing.hard.local_execution_required",
            Self::RetentionProhibited => "routing.hard.retention_prohibited",
            Self::RetentionLimitExceeded => "routing.hard.retention_limit_exceeded",
            Self::TrainingUseProhibited => "routing.hard.training_use_prohibited",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernedCandidateOutcome {
    Eligible,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernedScoreComponentKind {
    Health,
    AvailableCapacity,
    CostEfficiency,
    LatencyEfficiency,
    RiskReduction,
    OperatorPriority,
    Affinity,
}

impl GovernedScoreComponentKind {
    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::Health => "routing.score.health",
            Self::AvailableCapacity => "routing.score.available_capacity",
            Self::CostEfficiency => "routing.score.cost_efficiency",
            Self::LatencyEfficiency => "routing.score.latency_efficiency",
            Self::RiskReduction => "routing.score.risk_reduction",
            Self::OperatorPriority => "routing.score.operator_priority",
            Self::Affinity => "routing.score.affinity",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GovernedScoreComponent {
    pub kind: GovernedScoreComponentKind,
    pub normalized_value: u16,
    pub weight: u16,
    pub weighted_value: u64,
}

impl fmt::Debug for GovernedScoreComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedScoreComponent")
            .field("reason_code", &self.kind.reason_code())
            .field("normalized_value", &"<bounded>")
            .field("weight", &"<bounded>")
            .field("weighted_value", &"<bounded>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernedScoreBreakdown {
    pub score_revision: u64,
    pub components: [GovernedScoreComponent; GOVERNED_SCORE_COMPONENT_COUNT],
    pub weighted_total: u64,
    pub weight_total: u16,
    pub score: u16,
}

impl fmt::Debug for GovernedScoreBreakdown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedScoreBreakdown")
            .field("score_revision", &"<redacted>")
            .field("components", &self.components)
            .field("weighted_total", &"<bounded>")
            .field("weight_total", &"<bounded>")
            .field("score", &self.score)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernedCandidateEvaluation {
    pub provider: ProviderId,
    pub descriptor_revision: u64,
    outcome: GovernedCandidateOutcome,
    rejection_reasons: Vec<GovernedHardFilterReason>,
    score_breakdown: Option<GovernedScoreBreakdown>,
}

impl GovernedCandidateEvaluation {
    pub const fn outcome(&self) -> GovernedCandidateOutcome {
        self.outcome
    }

    pub fn rejection_reasons(&self) -> &[GovernedHardFilterReason] {
        &self.rejection_reasons
    }

    pub const fn score_breakdown(&self) -> Option<&GovernedScoreBreakdown> {
        self.score_breakdown.as_ref()
    }
}

impl fmt::Debug for GovernedCandidateEvaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedCandidateEvaluation")
            .field("provider", &self.provider)
            .field("descriptor_revision", &"<redacted>")
            .field("outcome", &self.outcome)
            .field("rejection_reasons", &self.rejection_reasons)
            .field("score_breakdown", &self.score_breakdown)
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GovernedRoutingSignals {
    /// `None` means no bounded runtime health sample is available.
    pub health: Option<u16>,
    pub load: u16,
    /// `None` means the provider does not expose a comparable quota window.
    pub quota_headroom: Option<u16>,
    pub cost: u16,
    pub latency: u16,
    pub risk: u16,
    pub priority: u16,
}

impl fmt::Debug for GovernedRoutingSignals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GovernedRoutingSignals(<bounded>)")
    }
}

impl GovernedRoutingSignals {
    fn is_valid(self) -> bool {
        [self.load, self.cost, self.latency, self.risk, self.priority]
            .into_iter()
            .all(|value| value <= ROUTING_SCORE_SCALE)
            && self.health.is_none_or(|value| value <= ROUTING_SCORE_SCALE)
            && self
                .quota_headroom
                .is_none_or(|value| value <= ROUTING_SCORE_SCALE)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GovernedRoutingWeights {
    pub health: u16,
    pub load: u16,
    pub cost: u16,
    pub latency: u16,
    pub risk: u16,
    pub priority: u16,
    pub affinity: u16,
}

impl fmt::Debug for GovernedRoutingWeights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GovernedRoutingWeights(<bounded>)")
    }
}

impl Default for GovernedRoutingWeights {
    fn default() -> Self {
        Self {
            health: 2_500,
            load: 1_500,
            cost: 1_000,
            latency: 1_000,
            risk: 1_500,
            priority: 1_000,
            affinity: 1_500,
        }
    }
}

impl GovernedRoutingWeights {
    fn total(self) -> Option<u64> {
        let weights = [
            self.health,
            self.load,
            self.cost,
            self.latency,
            self.risk,
            self.priority,
            self.affinity,
        ];
        if weights.iter().any(|weight| *weight > ROUTING_SCORE_SCALE) {
            return None;
        }
        let total = weights.into_iter().map(u64::from).sum::<u64>();
        (total > 0 && total <= u64::from(ROUTING_SCORE_SCALE)).then_some(total)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernedProviderDescriptor {
    pub revision: u64,
    pub pricing_revision: u64,
    pub tenant: TenantContext,
    pub provider: ProviderId,
    pub credential_ref: SecretRef,
    pub credential_available: bool,
    pub enabled: bool,
    pub revoked: bool,
    pub circuit_open: bool,
    pub quota_available: bool,
    pub inflight_cap_reached: bool,
    pub local_execution: bool,
    pub trust_tier: ProviderTrustTier,
    pub maximum_classification: DataClassification,
    pub capabilities: CapabilitySet,
    pub regions: Vec<PolicySelector>,
    pub retention_seconds: u32,
    pub training_use: bool,
    pub signals: GovernedRoutingSignals,
}

impl fmt::Debug for GovernedProviderDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedProviderDescriptor")
            .field("revision", &"<redacted>")
            .field("pricing_revision", &"<redacted>")
            .field("tenant", &"<redacted>")
            .field("provider", &self.provider)
            .field("credential_ref", &"<redacted>")
            .field("capability_count", &self.capabilities.as_slice().len())
            .field("region_count", &self.regions.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernedProviderRegistry {
    pub revision: u64,
    pub providers: Vec<GovernedProviderDescriptor>,
}

impl fmt::Debug for GovernedProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedProviderRegistry")
            .field("revision", &"<redacted>")
            .field("provider_count", &self.providers.len())
            .finish()
    }
}

pub struct GovernedRoutingRequest<'a> {
    pub tenant: TenantContext,
    pub classification: DataClassification,
    pub required_capabilities: &'a CapabilitySet,
    pub policy: &'a PolicyDecision,
    pub registry: &'a GovernedProviderRegistry,
    pub score_revision: u64,
    pub weights: GovernedRoutingWeights,
    pub affinity_provider: Option<ProviderId>,
    pub max_fallbacks: usize,
}

impl fmt::Debug for GovernedRoutingRequest<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedRoutingRequest")
            .field("tenant", &"<redacted>")
            .field("classification", &self.classification)
            .field("required_capabilities", &self.required_capabilities)
            .field("policy", &self.policy)
            .field("registry", &self.registry)
            .field("score_revision", &"<redacted>")
            .field("weights", &self.weights)
            .field("affinity_provider", &self.affinity_provider)
            .field("max_fallbacks", &self.max_fallbacks)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernedRoute {
    pub provider: ProviderId,
    pub descriptor_revision: u64,
    pub pricing_revision: u64,
    pub credential_ref: SecretRef,
    pub score: u16,
    pub score_breakdown: GovernedScoreBreakdown,
}

impl fmt::Debug for GovernedRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedRoute")
            .field("provider", &self.provider)
            .field("descriptor_revision", &"<redacted>")
            .field("pricing_revision", &"<redacted>")
            .field("credential_ref", &"<redacted>")
            .field("score", &self.score)
            .field("score_breakdown", &self.score_breakdown)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernedRoutingPlan {
    pub tenant: TenantContext,
    pub registry_revision: u64,
    pub score_revision: u64,
    pub policy_revision: PolicyRevisionId,
    pub primary: GovernedRoute,
    pub fallbacks: Vec<GovernedRoute>,
    pub candidate_evaluations: Vec<GovernedCandidateEvaluation>,
}

impl fmt::Debug for GovernedRoutingPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedRoutingPlan")
            .field("tenant", &"<redacted>")
            .field("registry_revision", &"<redacted>")
            .field("score_revision", &"<redacted>")
            .field("policy_revision", &"<redacted>")
            .field("primary", &self.primary)
            .field("fallback_count", &self.fallbacks.len())
            .field("candidate_count", &self.candidate_evaluations.len())
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GovernedRoutingError {
    PolicyDenied,
    ApprovalRequired,
    InvalidRegistryRevision,
    CandidateLimitExceeded { count: usize },
    InvalidDescriptorRevision { provider: ProviderId },
    DuplicateProvider { provider: ProviderId },
    RegionLimitExceeded { provider: ProviderId, count: usize },
    InvalidCredentialReference { provider: ProviderId },
    InvalidSignal { provider: ProviderId },
    InvalidWeights,
    FallbackLimitExceeded { requested: usize },
    NoEligibleProvider,
}

impl fmt::Debug for GovernedRoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PolicyDenied => "GovernedRoutingError::PolicyDenied",
            Self::ApprovalRequired => "GovernedRoutingError::ApprovalRequired",
            Self::InvalidRegistryRevision => "GovernedRoutingError::InvalidRegistryRevision",
            Self::CandidateLimitExceeded { .. } => "GovernedRoutingError::CandidateLimitExceeded",
            Self::InvalidDescriptorRevision { .. } => {
                "GovernedRoutingError::InvalidDescriptorRevision"
            }
            Self::DuplicateProvider { .. } => "GovernedRoutingError::DuplicateProvider",
            Self::RegionLimitExceeded { .. } => "GovernedRoutingError::RegionLimitExceeded",
            Self::InvalidCredentialReference { .. } => {
                "GovernedRoutingError::InvalidCredentialReference"
            }
            Self::InvalidSignal { .. } => "GovernedRoutingError::InvalidSignal",
            Self::InvalidWeights => "GovernedRoutingError::InvalidWeights",
            Self::FallbackLimitExceeded { .. } => "GovernedRoutingError::FallbackLimitExceeded",
            Self::NoEligibleProvider => "GovernedRoutingError::NoEligibleProvider",
        })
    }
}

impl fmt::Display for GovernedRoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PolicyDenied => "provider routing is denied by policy",
            Self::ApprovalRequired => "provider routing requires approval",
            Self::NoEligibleProvider => "no provider is eligible for this request",
            _ => "provider routing input is invalid",
        })
    }
}

impl Error for GovernedRoutingError {}

pub fn plan_governed_provider_route(
    request: &GovernedRoutingRequest<'_>,
) -> Result<GovernedRoutingPlan, GovernedRoutingError> {
    validate_request(request)?;

    let mut routes = Vec::new();
    let mut candidate_evaluations = Vec::with_capacity(request.registry.providers.len());
    for provider in &request.registry.providers {
        let rejection_reasons = provider_rejection_reasons(provider, request);
        if rejection_reasons.is_empty() {
            let score_breakdown = score_provider(provider, request);
            routes.push(GovernedRoute {
                provider: provider.provider,
                descriptor_revision: provider.revision,
                pricing_revision: provider.pricing_revision,
                credential_ref: provider.credential_ref.clone(),
                score: score_breakdown.score,
                score_breakdown: score_breakdown.clone(),
            });
            candidate_evaluations.push(GovernedCandidateEvaluation {
                provider: provider.provider,
                descriptor_revision: provider.revision,
                outcome: GovernedCandidateOutcome::Eligible,
                rejection_reasons,
                score_breakdown: Some(score_breakdown),
            });
        } else {
            candidate_evaluations.push(GovernedCandidateEvaluation {
                provider: provider.provider,
                descriptor_revision: provider.revision,
                outcome: GovernedCandidateOutcome::Rejected,
                rejection_reasons,
                score_breakdown: None,
            });
        }
    }
    routes.sort_by(|left, right| {
        let left_has_affinity = request.affinity_provider == Some(left.provider);
        let right_has_affinity = request.affinity_provider == Some(right.provider);
        right_has_affinity.cmp(&left_has_affinity).then_with(|| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.provider.cmp(&right.provider))
        })
    });

    let primary = routes
        .first()
        .cloned()
        .ok_or(GovernedRoutingError::NoEligibleProvider)?;
    let fallbacks = routes
        .into_iter()
        .skip(1)
        .take(request.max_fallbacks)
        .collect();
    Ok(GovernedRoutingPlan {
        tenant: request.tenant,
        registry_revision: request.registry.revision,
        score_revision: request.score_revision,
        policy_revision: request.policy.policy_revision,
        primary,
        fallbacks,
        candidate_evaluations,
    })
}

fn validate_request(request: &GovernedRoutingRequest<'_>) -> Result<(), GovernedRoutingError> {
    match request.policy.effect {
        PolicyEffect::Deny => return Err(GovernedRoutingError::PolicyDenied),
        PolicyEffect::RequireApproval => return Err(GovernedRoutingError::ApprovalRequired),
        PolicyEffect::Allow => {}
    }
    if request
        .policy
        .obligations
        .contains(&GovernanceObligation::RequireHumanApproval)
    {
        return Err(GovernedRoutingError::ApprovalRequired);
    }
    if request.registry.revision == 0 {
        return Err(GovernedRoutingError::InvalidRegistryRevision);
    }
    if request.score_revision == 0 {
        return Err(GovernedRoutingError::InvalidWeights);
    }
    if request.registry.providers.len() > MAX_GOVERNED_ROUTING_CANDIDATES {
        return Err(GovernedRoutingError::CandidateLimitExceeded {
            count: request.registry.providers.len(),
        });
    }
    if request.max_fallbacks > MAX_GOVERNED_ROUTING_FALLBACKS {
        return Err(GovernedRoutingError::FallbackLimitExceeded {
            requested: request.max_fallbacks,
        });
    }
    if request.weights.total().is_none() {
        return Err(GovernedRoutingError::InvalidWeights);
    }

    for (index, provider) in request.registry.providers.iter().enumerate() {
        if provider.revision == 0 || provider.pricing_revision == 0 {
            return Err(GovernedRoutingError::InvalidDescriptorRevision {
                provider: provider.provider,
            });
        }
        if provider.regions.len() > MAX_GOVERNED_PROVIDER_REGIONS {
            return Err(GovernedRoutingError::RegionLimitExceeded {
                provider: provider.provider,
                count: provider.regions.len(),
            });
        }
        if !provider.credential_ref.is_well_formed() {
            return Err(GovernedRoutingError::InvalidCredentialReference {
                provider: provider.provider,
            });
        }
        if !provider.signals.is_valid() {
            return Err(GovernedRoutingError::InvalidSignal {
                provider: provider.provider,
            });
        }
        if request.registry.providers[..index]
            .iter()
            .any(|other| other.tenant == provider.tenant && other.provider == provider.provider)
        {
            return Err(GovernedRoutingError::DuplicateProvider {
                provider: provider.provider,
            });
        }
    }
    Ok(())
}

fn provider_rejection_reasons(
    provider: &GovernedProviderDescriptor,
    request: &GovernedRoutingRequest<'_>,
) -> Vec<GovernedHardFilterReason> {
    let obligations = &request.policy.obligations;
    let mut reasons = Vec::with_capacity(MAX_GOVERNED_HARD_FILTER_REASONS);
    if provider.tenant != request.tenant {
        reasons.push(GovernedHardFilterReason::TenantMismatch);
    }
    if !provider.enabled {
        reasons.push(GovernedHardFilterReason::ProviderDisabled);
    }
    if provider.revoked {
        reasons.push(GovernedHardFilterReason::ProviderRevoked);
    }
    // Continuations retain hard affinity across temporary runtime degradation. Static
    // eligibility (including revocation and policy) remains authoritative.
    let has_hard_affinity = request.affinity_provider == Some(provider.provider);
    if provider.circuit_open && !has_hard_affinity {
        reasons.push(GovernedHardFilterReason::CircuitOpen);
    }
    if !provider.quota_available && !has_hard_affinity {
        reasons.push(GovernedHardFilterReason::QuotaUnavailable);
    }
    if provider.inflight_cap_reached && !has_hard_affinity {
        reasons.push(GovernedHardFilterReason::InFlightCapReached);
    }
    if !provider.credential_available {
        reasons.push(GovernedHardFilterReason::CredentialUnavailable);
    }
    if request.classification > provider.maximum_classification {
        reasons.push(GovernedHardFilterReason::ClassificationUnsupported);
    }
    if !request
        .required_capabilities
        .missing_from(&provider.capabilities)
        .is_empty()
    {
        reasons.push(GovernedHardFilterReason::CapabilityMissing);
    }

    let has_allow_list = obligations
        .iter()
        .any(|item| matches!(item, GovernanceObligation::AllowProvider(_)));
    let allow_list_match = obligations.iter().any(|item| {
        matches!(item, GovernanceObligation::AllowProvider(selector) if selector_matches_provider(selector, provider.provider))
    });
    let denied = obligations.iter().any(|item| {
        matches!(item, GovernanceObligation::DenyProvider(selector) if selector_matches_provider(selector, provider.provider))
    });
    let minimum_trust = obligations.iter().filter_map(|item| match item {
        GovernanceObligation::MinimumProviderTrust(tier) => Some(*tier),
        _ => None,
    });
    let regions_match = obligations.iter().all(|item| match item {
        GovernanceObligation::RequireRegion(required) => provider
            .regions
            .iter()
            .any(|offered| selectors_overlap(required, offered)),
        _ => true,
    });
    let maximum_retention = obligations.iter().filter_map(|item| match item {
        GovernanceObligation::RetentionSeconds(seconds) => Some(*seconds),
        _ => None,
    });

    if has_allow_list && !allow_list_match {
        reasons.push(GovernedHardFilterReason::ProviderNotAllowed);
    }
    if denied {
        reasons.push(GovernedHardFilterReason::ProviderDenied);
    }
    if minimum_trust
        .max()
        .is_some_and(|required| provider.trust_tier < required)
    {
        reasons.push(GovernedHardFilterReason::TrustTierInsufficient);
    }
    if !regions_match {
        reasons.push(GovernedHardFilterReason::RegionUnavailable);
    }
    if obligations.contains(&GovernanceObligation::RequireLocalExecution)
        && !provider.local_execution
    {
        reasons.push(GovernedHardFilterReason::LocalExecutionRequired);
    }
    if obligations.contains(&GovernanceObligation::ProhibitRetention)
        && provider.retention_seconds != 0
    {
        reasons.push(GovernedHardFilterReason::RetentionProhibited);
    }
    if maximum_retention
        .min()
        .is_some_and(|limit| provider.retention_seconds > limit)
    {
        reasons.push(GovernedHardFilterReason::RetentionLimitExceeded);
    }
    if obligations.contains(&GovernanceObligation::ProhibitTrainingUse) && provider.training_use {
        reasons.push(GovernedHardFilterReason::TrainingUseProhibited);
    }
    assert!(reasons.len() <= MAX_GOVERNED_HARD_FILTER_REASONS);
    reasons
}

fn selector_matches_provider(selector: &PolicySelector, provider: ProviderId) -> bool {
    selector.as_str() == "*" || selector.as_str() == provider.label()
}

fn selectors_overlap(left: &PolicySelector, right: &PolicySelector) -> bool {
    left.as_str() == "*" || right.as_str() == "*" || left.as_str() == right.as_str()
}

fn score_provider(
    provider: &GovernedProviderDescriptor,
    request: &GovernedRoutingRequest<'_>,
) -> GovernedScoreBreakdown {
    let weights = request.weights;
    let inverse = |value: u16| ROUTING_SCORE_SCALE - value;
    let affinity = if request.affinity_provider == Some(provider.provider) {
        ROUTING_SCORE_SCALE
    } else {
        0
    };
    let components = [
        score_component(
            GovernedScoreComponentKind::Health,
            provider.signals.health.unwrap_or(ROUTING_SCORE_SCALE / 2),
            weights.health,
        ),
        score_component(
            GovernedScoreComponentKind::AvailableCapacity,
            provider.signals.quota_headroom.map_or_else(
                || inverse(provider.signals.load),
                |quota| quota.min(inverse(provider.signals.load)),
            ),
            weights.load,
        ),
        score_component(
            GovernedScoreComponentKind::CostEfficiency,
            inverse(provider.signals.cost),
            weights.cost,
        ),
        score_component(
            GovernedScoreComponentKind::LatencyEfficiency,
            inverse(provider.signals.latency),
            weights.latency,
        ),
        score_component(
            GovernedScoreComponentKind::RiskReduction,
            inverse(provider.signals.risk),
            weights.risk,
        ),
        score_component(
            GovernedScoreComponentKind::OperatorPriority,
            provider.signals.priority,
            weights.priority,
        ),
        score_component(
            GovernedScoreComponentKind::Affinity,
            affinity,
            weights.affinity,
        ),
    ];
    let weighted_total = components
        .iter()
        .map(|component| component.weighted_value)
        .sum::<u64>();
    let weight_total = request.weights.total().unwrap_or(1) as u16;
    GovernedScoreBreakdown {
        score_revision: request.score_revision,
        components,
        weighted_total,
        weight_total,
        score: (weighted_total / u64::from(weight_total)) as u16,
    }
}

fn score_component(
    kind: GovernedScoreComponentKind,
    normalized_value: u16,
    weight: u16,
) -> GovernedScoreComponent {
    GovernedScoreComponent {
        kind,
        normalized_value,
        weight,
        weighted_value: u64::from(normalized_value) * u64::from(weight),
    }
}
