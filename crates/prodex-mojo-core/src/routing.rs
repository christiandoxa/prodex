#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreInput {
    pub health: i64,
    pub load: i64,
    pub quota_headroom: i64,
    pub quota_present: bool,
    pub cost: i64,
    pub latency: i64,
    pub risk: i64,
    pub priority: i64,
    pub affinity: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreWeights {
    pub health: i64,
    pub load: i64,
    pub cost: i64,
    pub latency: i64,
    pub risk: i64,
    pub priority: i64,
    pub affinity: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    pub components: [u16; 7],
    pub weighted_total: u64,
    pub score: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutingPlanInput {
    pub hard_eligible: bool,
    pub capability_mask: u8,
    pub provider_order: i64,
    pub score: ScoreInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingPlan {
    pub eligible: Vec<bool>,
    pub reason_tags: Vec<u8>,
    pub scores: Vec<Score>,
    pub ordered_indices: Vec<usize>,
}

pub const ROUTING_REASON_ELIGIBLE: u8 = 0;
pub const ROUTING_REASON_HARD_REJECTED: u8 = 1;
pub const ROUTING_REASON_CAPABILITY_MISSING: u8 = 2;
const SCORE_SCALE: i64 = 10_000;
pub const CAPABILITY_REASON_MALFORMED: u8 = 0;
pub const CAPABILITY_REASON_MISSING: u8 = 2;
pub const CAPABILITY_REASON_COMPATIBLE: u8 = 1;
pub const ABI_VERSION: u32 = 1;

pub fn self_test() -> bool {
    let plan = routing_plan_batch(
        &[RoutingPlanInput {
            hard_eligible: true,
            capability_mask: 1,
            provider_order: 0,
            score: ScoreInput {
                health: 10_000,
                load: 0,
                quota_headroom: 10_000,
                quota_present: true,
                cost: 0,
                latency: 0,
                risk: 0,
                priority: 10_000,
                affinity: true,
            },
        }],
        1,
        ScoreWeights {
            health: 10_000,
            load: 0,
            cost: 0,
            latency: 0,
            risk: 0,
            priority: 0,
            affinity: 0,
        },
    );
    let capability = capability_match_batch(&[true, true], &[1, 0], 1);
    plan.is_ok_and(|plan| {
        plan.eligible == [true]
            && plan.reason_tags == [ROUTING_REASON_ELIGIBLE]
            && plan.ordered_indices == [0]
    }) && capability.is_ok_and(|result| {
        result.first_compatible == Some(0)
            && result.first_incompatible == Some(1)
            && result.compatible == [true, false]
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityMatch {
    pub compatible: Vec<bool>,
    pub reason_tags: Vec<u8>,
    pub first_compatible: Option<usize>,
    pub first_incompatible: Option<usize>,
}

unsafe extern "C" {
    fn prodex_mojo_abi_version() -> i64;
    fn prodex_routing_plan_batch(
        hard_eligible: *const i64,
        capability_masks: *const i64,
        provider_order: *const i64,
        health: *const i64,
        load: *const i64,
        quota_headroom: *const i64,
        quota_present: *const i64,
        cost: *const i64,
        latency: *const i64,
        risk: *const i64,
        priority: *const i64,
        affinity: *const i64,
        eligible: *mut i64,
        reason_tags: *mut i64,
        normalized_values: *mut i64,
        weighted_totals: *mut i64,
        scores: *mut i64,
        ordered_indices: *mut i64,
        ordered_count: *mut i64,
        count: i64,
        required_capability_mask: i64,
        health_weight: i64,
        load_weight: i64,
        cost_weight: i64,
        latency_weight: i64,
        risk_weight: i64,
        priority_weight: i64,
        affinity_weight: i64,
    ) -> i64;
    fn prodex_capability_match_batch(
        well_formed: *const i64,
        capability_masks: *const i64,
        compatible: *mut i64,
        reason_tags: *mut i64,
        first_compatible: *mut i64,
        first_incompatible: *mut i64,
        count: i64,
        required_capability_mask: i64,
    ) -> i64;
}

pub fn abi_version() -> Result<u32, crate::MojoError> {
    let version = u32::try_from(unsafe { prodex_mojo_abi_version() })
        .map_err(|_| crate::MojoError::AbiMismatch)?;
    (version == ABI_VERSION)
        .then_some(version)
        .ok_or(crate::MojoError::AbiMismatch)
}

pub fn capability_match_batch(
    well_formed: &[bool],
    capability_masks: &[u8],
    required_capability_mask: u8,
) -> Result<CapabilityMatch, crate::MojoError> {
    if well_formed.len() != capability_masks.len() {
        return Err(crate::MojoError::InvalidInput);
    }
    capability_match_batch_impl(well_formed, capability_masks, required_capability_mask)
        .ok_or(crate::MojoError::InvalidOutput)
}

fn capability_match_batch_impl(
    well_formed: &[bool],
    capability_masks: &[u8],
    required_capability_mask: u8,
) -> Option<CapabilityMatch> {
    {
        let well_formed = well_formed
            .iter()
            .map(|value| i64::from(*value))
            .collect::<Vec<_>>();
        let capability_masks = capability_masks
            .iter()
            .map(|value| i64::from(*value))
            .collect::<Vec<_>>();
        let mut compatible = vec![0_i64; well_formed.len()];
        let mut reason_tags = vec![0_i64; well_formed.len()];
        let mut first_compatible = -1_i64;
        let mut first_incompatible = -1_i64;
        let status = unsafe {
            prodex_capability_match_batch(
                well_formed.as_ptr(),
                capability_masks.as_ptr(),
                compatible.as_mut_ptr(),
                reason_tags.as_mut_ptr(),
                &mut first_compatible,
                &mut first_incompatible,
                i64::try_from(well_formed.len()).ok()?,
                i64::from(required_capability_mask),
            )
        };
        if status != 0 {
            return None;
        }
        let to_index = |value: i64| {
            (value >= 0)
                .then(|| usize::try_from(value).ok())
                .flatten()
                .filter(|index| *index < well_formed.len())
        };
        if first_compatible < -1
            || first_incompatible < -1
            || (first_compatible >= 0 && to_index(first_compatible).is_none())
            || (first_incompatible >= 0 && to_index(first_incompatible).is_none())
        {
            return None;
        }
        let first_compatible = to_index(first_compatible);
        let first_incompatible = to_index(first_incompatible);
        let expected = well_formed
            .iter()
            .zip(&capability_masks)
            .map(|(well_formed, mask)| {
                if *well_formed == 0 {
                    (false, CAPABILITY_REASON_MALFORMED)
                } else if (*mask & i64::from(required_capability_mask))
                    == i64::from(required_capability_mask)
                {
                    (true, CAPABILITY_REASON_COMPATIBLE)
                } else {
                    (false, CAPABILITY_REASON_MISSING)
                }
            })
            .collect::<Vec<_>>();
        if compatible.iter().zip(&reason_tags).zip(&expected).any(
            |((compatible, reason), (expected_compatible, expected_reason))| {
                !matches!(*compatible, 0 | 1)
                    || *compatible != i64::from(*expected_compatible)
                    || *reason != i64::from(*expected_reason)
            },
        ) {
            return None;
        }
        let expected_first_compatible = expected.iter().position(|(compatible, _)| *compatible);
        let expected_first_incompatible = expected
            .iter()
            .position(|(compatible, reason)| !*compatible && *reason == CAPABILITY_REASON_MISSING);
        if first_compatible != expected_first_compatible
            || first_incompatible != expected_first_incompatible
        {
            return None;
        }
        Some(CapabilityMatch {
            compatible: compatible.into_iter().map(|value| value == 1).collect(),
            reason_tags: reason_tags
                .into_iter()
                .map(u8::try_from)
                .collect::<Result<Vec<_>, _>>()
                .ok()?,
            first_compatible,
            first_incompatible,
        })
    }
}

pub fn routing_plan_batch(
    inputs: &[RoutingPlanInput],
    required_capability_mask: u8,
    weights: ScoreWeights,
) -> Result<RoutingPlan, crate::MojoError> {
    routing_plan_batch_impl(inputs, required_capability_mask, weights)
        .ok_or(crate::MojoError::InvalidOutput)
}

fn routing_plan_batch_impl(
    inputs: &[RoutingPlanInput],
    required_capability_mask: u8,
    weights: ScoreWeights,
) -> Option<RoutingPlan> {
    {
        let hard_eligible = inputs
            .iter()
            .map(|input| i64::from(input.hard_eligible))
            .collect::<Vec<_>>();
        let capability_masks = inputs
            .iter()
            .map(|input| i64::from(input.capability_mask))
            .collect::<Vec<_>>();
        let provider_order = inputs
            .iter()
            .map(|input| input.provider_order)
            .collect::<Vec<_>>();
        let health = inputs
            .iter()
            .map(|input| input.score.health)
            .collect::<Vec<_>>();
        let load = inputs
            .iter()
            .map(|input| input.score.load)
            .collect::<Vec<_>>();
        let quota_headroom = inputs
            .iter()
            .map(|input| input.score.quota_headroom)
            .collect::<Vec<_>>();
        let quota_present = inputs
            .iter()
            .map(|input| i64::from(input.score.quota_present))
            .collect::<Vec<_>>();
        let cost = inputs
            .iter()
            .map(|input| input.score.cost)
            .collect::<Vec<_>>();
        let latency = inputs
            .iter()
            .map(|input| input.score.latency)
            .collect::<Vec<_>>();
        let risk = inputs
            .iter()
            .map(|input| input.score.risk)
            .collect::<Vec<_>>();
        let priority = inputs
            .iter()
            .map(|input| input.score.priority)
            .collect::<Vec<_>>();
        let affinity = inputs
            .iter()
            .map(|input| i64::from(input.score.affinity))
            .collect::<Vec<_>>();
        let mut eligible = vec![0_i64; inputs.len()];
        let mut reason_tags = vec![0_i64; inputs.len()];
        let mut normalized_values = vec![0_i64; inputs.len() * 7];
        let mut weighted_totals = vec![0_i64; inputs.len()];
        let mut scores = vec![0_i64; inputs.len()];
        let mut ordered_indices = vec![0_i64; inputs.len()];
        let mut ordered_count = 0_i64;
        let status = unsafe {
            prodex_routing_plan_batch(
                hard_eligible.as_ptr(),
                capability_masks.as_ptr(),
                provider_order.as_ptr(),
                health.as_ptr(),
                load.as_ptr(),
                quota_headroom.as_ptr(),
                quota_present.as_ptr(),
                cost.as_ptr(),
                latency.as_ptr(),
                risk.as_ptr(),
                priority.as_ptr(),
                affinity.as_ptr(),
                eligible.as_mut_ptr(),
                reason_tags.as_mut_ptr(),
                normalized_values.as_mut_ptr(),
                weighted_totals.as_mut_ptr(),
                scores.as_mut_ptr(),
                ordered_indices.as_mut_ptr(),
                &mut ordered_count,
                i64::try_from(inputs.len()).ok()?,
                i64::from(required_capability_mask),
                weights.health,
                weights.load,
                weights.cost,
                weights.latency,
                weights.risk,
                weights.priority,
                weights.affinity,
            )
        };
        if status != 0 || !(0..=i64::try_from(inputs.len()).ok()?).contains(&ordered_count) {
            return None;
        }
        let expected = inputs
            .iter()
            .map(|input| {
                let eligible = input.hard_eligible
                    && input.capability_mask & required_capability_mask == required_capability_mask;
                let reason = if eligible {
                    ROUTING_REASON_ELIGIBLE
                } else if !input.hard_eligible {
                    ROUTING_REASON_HARD_REJECTED
                } else {
                    ROUTING_REASON_CAPABILITY_MISSING
                };
                (eligible, reason)
            })
            .collect::<Vec<_>>();
        if eligible.iter().zip(&reason_tags).zip(&expected).any(
            |((eligible, reason), (expected_eligible, expected_reason))| {
                !matches!(*eligible, 0 | 1)
                    || *eligible != i64::from(*expected_eligible)
                    || u8::try_from(*reason).ok() != Some(*expected_reason)
            },
        ) {
            return None;
        }
        let scores = inputs
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let base = index * 7;
                if normalized_values[base..base + 7]
                    .iter()
                    .any(|value| !(0..=SCORE_SCALE).contains(value))
                    || weighted_totals[index] < 0
                    || scores[index] < 0
                    || scores[index] > SCORE_SCALE
                {
                    return None;
                }
                let components = normalized_values[base..base + 7]
                    .iter()
                    .copied()
                    .map(u16::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?
                    .try_into()
                    .ok()?;
                Some(Score {
                    components,
                    weighted_total: u64::try_from(weighted_totals[index]).ok()?,
                    score: u16::try_from(scores[index]).ok()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let ordered_indices = ordered_indices
            .into_iter()
            .take(usize::try_from(ordered_count).ok()?)
            .map(usize::try_from)
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        if ordered_indices.len() != expected.iter().filter(|(eligible, _)| *eligible).count() {
            return None;
        }
        let mut seen = vec![false; inputs.len()];
        if ordered_indices.iter().any(|&index| {
            index >= inputs.len() || !expected[index].0 || std::mem::replace(&mut seen[index], true)
        }) {
            return None;
        }
        Some(RoutingPlan {
            eligible: eligible.into_iter().map(|value| value == 1).collect(),
            reason_tags: reason_tags
                .into_iter()
                .map(u8::try_from)
                .collect::<Result<Vec<_>, _>>()
                .ok()?,
            scores,
            ordered_indices,
        })
    }
}
