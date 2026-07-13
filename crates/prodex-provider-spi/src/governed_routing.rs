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

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GovernedRoutingSignals {
    pub health: u16,
    pub load: u16,
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
        [
            self.health,
            self.load,
            self.cost,
            self.latency,
            self.risk,
            self.priority,
        ]
        .into_iter()
        .all(|value| value <= ROUTING_SCORE_SCALE)
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
    pub tenant: TenantContext,
    pub provider: ProviderId,
    pub credential_ref: SecretRef,
    pub credential_available: bool,
    pub enabled: bool,
    pub revoked: bool,
    pub circuit_open: bool,
    pub quota_available: bool,
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
    pub credential_ref: SecretRef,
    pub score: u16,
}

impl fmt::Debug for GovernedRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernedRoute")
            .field("provider", &self.provider)
            .field("descriptor_revision", &"<redacted>")
            .field("credential_ref", &"<redacted>")
            .field("score", &self.score)
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

    let mut routes = request
        .registry
        .providers
        .iter()
        .filter(|provider| provider.tenant == request.tenant)
        .filter(|provider| provider_is_eligible(provider, request))
        .map(|provider| GovernedRoute {
            provider: provider.provider,
            descriptor_revision: provider.revision,
            credential_ref: provider.credential_ref.clone(),
            score: score_provider(provider, request),
        })
        .collect::<Vec<_>>();
    routes.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.provider.cmp(&right.provider))
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
        if provider.revision == 0 {
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

fn provider_is_eligible(
    provider: &GovernedProviderDescriptor,
    request: &GovernedRoutingRequest<'_>,
) -> bool {
    provider.enabled
        && !provider.revoked
        && !provider.circuit_open
        && provider.quota_available
        && provider.credential_available
        && request.classification <= provider.maximum_classification
        && request
            .required_capabilities
            .missing_from(&provider.capabilities)
            .is_empty()
        && obligations_allow(provider, &request.policy.obligations)
}

fn obligations_allow(
    provider: &GovernedProviderDescriptor,
    obligations: &[GovernanceObligation],
) -> bool {
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

    (!has_allow_list || allow_list_match)
        && !denied
        && minimum_trust
            .max()
            .is_none_or(|required| provider.trust_tier >= required)
        && regions_match
        && (!obligations.contains(&GovernanceObligation::RequireLocalExecution)
            || provider.local_execution)
        && (!obligations.contains(&GovernanceObligation::ProhibitRetention)
            || provider.retention_seconds == 0)
        && maximum_retention
            .min()
            .is_none_or(|limit| provider.retention_seconds <= limit)
        && (!obligations.contains(&GovernanceObligation::ProhibitTrainingUse)
            || !provider.training_use)
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
) -> u16 {
    let weights = request.weights;
    let inverse = |value: u16| ROUTING_SCORE_SCALE - value;
    let affinity = if request.affinity_provider == Some(provider.provider) {
        ROUTING_SCORE_SCALE
    } else {
        0
    };
    let numerator = [
        (provider.signals.health, weights.health),
        (inverse(provider.signals.load), weights.load),
        (inverse(provider.signals.cost), weights.cost),
        (inverse(provider.signals.latency), weights.latency),
        (inverse(provider.signals.risk), weights.risk),
        (provider.signals.priority, weights.priority),
        (affinity, weights.affinity),
    ]
    .into_iter()
    .map(|(value, weight)| u64::from(value) * u64::from(weight))
    .sum::<u64>();
    (numerator / request.weights.total().unwrap_or(1)) as u16
}
