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
#[cfg(not(prodex_mojo_fallback))]
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
    let score = score_batch(
        &[ScoreInput {
            health: 10_000,
            load: 0,
            quota_headroom: 10_000,
            quota_present: true,
            cost: 0,
            latency: 0,
            risk: 0,
            priority: 10_000,
            affinity: true,
        }],
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
    plan.is_some_and(|plan| {
        plan.eligible == [true]
            && plan.reason_tags == [ROUTING_REASON_ELIGIBLE]
            && plan.ordered_indices == [0]
    }) && capability.is_some_and(|result| {
        result.first_compatible == Some(0)
            && result.first_incompatible == Some(1)
            && result.compatible == [true, false]
    }) && score.is_some_and(|scores| scores[0].score == 10_000)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityMatch {
    pub compatible: Vec<bool>,
    pub reason_tags: Vec<u8>,
    pub first_compatible: Option<usize>,
    pub first_incompatible: Option<usize>,
}

#[cfg(not(prodex_mojo_fallback))]
unsafe extern "C" {
    fn prodex_mojo_abi_version() -> i64;
    fn prodex_routing_score_batch(
        health: *const i64,
        load: *const i64,
        quota_headroom: *const i64,
        quota_present: *const i64,
        cost: *const i64,
        latency: *const i64,
        risk: *const i64,
        priority: *const i64,
        affinity: *const i64,
        normalized_values: *mut i64,
        weighted_totals: *mut i64,
        scores: *mut i64,
        count: i64,
        health_weight: i64,
        load_weight: i64,
        cost_weight: i64,
        latency_weight: i64,
        risk_weight: i64,
        priority_weight: i64,
        affinity_weight: i64,
    ) -> i64;
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

pub fn abi_version() -> Option<u32> {
    #[cfg(not(prodex_mojo_fallback))]
    {
        u32::try_from(unsafe { prodex_mojo_abi_version() }).ok()
    }
    #[cfg(prodex_mojo_fallback)]
    Some(ABI_VERSION)
}

pub fn capability_match_batch(
    well_formed: &[bool],
    capability_masks: &[u8],
    required_capability_mask: u8,
) -> Option<CapabilityMatch> {
    if well_formed.len() != capability_masks.len() {
        return None;
    }
    capability_match_batch_impl(well_formed, capability_masks, required_capability_mask)
}

#[cfg(not(prodex_mojo_fallback))]
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

#[cfg(prodex_mojo_fallback)]
fn capability_match_batch_impl(
    well_formed: &[bool],
    capability_masks: &[u8],
    required_capability_mask: u8,
) -> Option<CapabilityMatch> {
    {
        let compatible = well_formed
            .iter()
            .zip(capability_masks)
            .map(|(well_formed, mask)| {
                *well_formed && (*mask & required_capability_mask) == required_capability_mask
            })
            .collect::<Vec<_>>();
        let reason_tags = well_formed
            .iter()
            .zip(&compatible)
            .map(|(well_formed, compatible)| {
                if !well_formed {
                    CAPABILITY_REASON_MALFORMED
                } else if *compatible {
                    CAPABILITY_REASON_COMPATIBLE
                } else {
                    CAPABILITY_REASON_MISSING
                }
            })
            .collect::<Vec<_>>();
        Some(CapabilityMatch {
            first_compatible: compatible.iter().position(|value| *value),
            first_incompatible: well_formed
                .iter()
                .zip(&compatible)
                .position(|(well_formed, compatible)| *well_formed && !*compatible),
            compatible,
            reason_tags,
        })
    }
}

pub fn routing_plan_batch(
    inputs: &[RoutingPlanInput],
    required_capability_mask: u8,
    weights: ScoreWeights,
) -> Option<RoutingPlan> {
    routing_plan_batch_impl(inputs, required_capability_mask, weights)
}

#[cfg(not(prodex_mojo_fallback))]
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

#[cfg(prodex_mojo_fallback)]
fn routing_plan_batch_impl(
    inputs: &[RoutingPlanInput],
    required_capability_mask: u8,
    weights: ScoreWeights,
) -> Option<RoutingPlan> {
    {
        let scores = inputs
            .iter()
            .map(|input| score_rust(input.score, weights))
            .collect::<Vec<_>>();
        let eligible = inputs
            .iter()
            .map(|input| {
                input.hard_eligible
                    && input.capability_mask & required_capability_mask == required_capability_mask
            })
            .collect::<Vec<_>>();
        let reason_tags = inputs
            .iter()
            .zip(&eligible)
            .map(|(input, eligible)| {
                if *eligible {
                    ROUTING_REASON_ELIGIBLE
                } else if !input.hard_eligible {
                    ROUTING_REASON_HARD_REJECTED
                } else {
                    ROUTING_REASON_CAPABILITY_MISSING
                }
            })
            .collect::<Vec<_>>();
        let mut ordered_indices = (0..inputs.len())
            .filter(|&index| eligible[index])
            .collect::<Vec<_>>();
        ordered_indices.sort_by(|&left, &right| {
            inputs[right]
                .score
                .affinity
                .cmp(&inputs[left].score.affinity)
                .then_with(|| scores[right].score.cmp(&scores[left].score))
                .then_with(|| {
                    inputs[left]
                        .provider_order
                        .cmp(&inputs[right].provider_order)
                })
                .then_with(|| left.cmp(&right))
        });
        Some(RoutingPlan {
            eligible,
            reason_tags,
            scores,
            ordered_indices,
        })
    }
}

pub fn score_batch(inputs: &[ScoreInput], weights: ScoreWeights) -> Option<Vec<Score>> {
    #[cfg(not(prodex_mojo_fallback))]
    {
        let health = inputs.iter().map(|input| input.health).collect::<Vec<_>>();
        let load = inputs.iter().map(|input| input.load).collect::<Vec<_>>();
        let quota_headroom = inputs
            .iter()
            .map(|input| input.quota_headroom)
            .collect::<Vec<_>>();
        let quota_present = inputs
            .iter()
            .map(|input| i64::from(input.quota_present))
            .collect::<Vec<_>>();
        let cost = inputs.iter().map(|input| input.cost).collect::<Vec<_>>();
        let latency = inputs.iter().map(|input| input.latency).collect::<Vec<_>>();
        let risk = inputs.iter().map(|input| input.risk).collect::<Vec<_>>();
        let priority = inputs
            .iter()
            .map(|input| input.priority)
            .collect::<Vec<_>>();
        let affinity = inputs
            .iter()
            .map(|input| i64::from(input.affinity))
            .collect::<Vec<_>>();
        let mut normalized_values = vec![0_i64; inputs.len() * 7];
        let mut weighted_totals = vec![0_i64; inputs.len()];
        let mut scores = vec![0_i64; inputs.len()];
        let status = unsafe {
            prodex_routing_score_batch(
                health.as_ptr(),
                load.as_ptr(),
                quota_headroom.as_ptr(),
                quota_present.as_ptr(),
                cost.as_ptr(),
                latency.as_ptr(),
                risk.as_ptr(),
                priority.as_ptr(),
                affinity.as_ptr(),
                normalized_values.as_mut_ptr(),
                weighted_totals.as_mut_ptr(),
                scores.as_mut_ptr(),
                i64::try_from(inputs.len()).ok()?,
                weights.health,
                weights.load,
                weights.cost,
                weights.latency,
                weights.risk,
                weights.priority,
                weights.affinity,
            )
        };
        if status == 0 {
            inputs
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    let base = index * 7;
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
                .collect()
        } else {
            None
        }
    }

    #[cfg(prodex_mojo_fallback)]
    Some(
        inputs
            .iter()
            .copied()
            .map(|input| score_rust(input, weights))
            .collect(),
    )
}

#[cfg(prodex_mojo_fallback)]
fn score_rust(input: ScoreInput, weights: ScoreWeights) -> Score {
    let inverse = |value: i64| 10_000 - value;
    let available_capacity = if input.quota_present {
        input.quota_headroom.min(inverse(input.load))
    } else {
        inverse(input.load)
    };
    let components = [
        input.health,
        available_capacity,
        inverse(input.cost),
        inverse(input.latency),
        inverse(input.risk),
        input.priority,
        if input.affinity { 10_000 } else { 0 },
    ];
    let weights = [
        weights.health,
        weights.load,
        weights.cost,
        weights.latency,
        weights.risk,
        weights.priority,
        weights.affinity,
    ];
    let weighted_total = components
        .iter()
        .zip(weights)
        .map(|(value, weight)| {
            u64::try_from(*value).unwrap_or(0) * u64::try_from(weight).unwrap_or(0)
        })
        .sum::<u64>();
    let weight_total = weights.into_iter().sum::<i64>().max(1) as u64;
    Score {
        components: components.map(|value| value as u16),
        weighted_total,
        score: (weighted_total / weight_total) as u16,
    }
}
