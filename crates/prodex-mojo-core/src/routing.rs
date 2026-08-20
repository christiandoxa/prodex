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

#[cfg(not(prodex_mojo_fallback))]
unsafe extern "C" {
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
