use super::GovernedRoutingWeights;
use super::scoring::{MojoRoutingScore, NormalizedRoutingScoreInput};

pub(super) fn score_batch(
    inputs: &[NormalizedRoutingScoreInput],
    weights: GovernedRoutingWeights,
) -> Option<Vec<MojoRoutingScore>> {
    let inputs = inputs
        .iter()
        .map(|input| prodex_mojo_core::routing::ScoreInput {
            health: input.health,
            load: input.load,
            quota_headroom: input.quota_headroom,
            quota_present: input.quota_present,
            cost: input.cost,
            latency: input.latency,
            risk: input.risk,
            priority: input.priority,
            affinity: input.affinity,
        })
        .collect::<Vec<_>>();
    let weights = prodex_mojo_core::routing::ScoreWeights {
        health: i64::from(weights.health),
        load: i64::from(weights.load),
        cost: i64::from(weights.cost),
        latency: i64::from(weights.latency),
        risk: i64::from(weights.risk),
        priority: i64::from(weights.priority),
        affinity: i64::from(weights.affinity),
    };
    prodex_mojo_core::routing::score_batch(&inputs, weights).map(|scores| {
        scores
            .into_iter()
            .map(|score| MojoRoutingScore {
                components: score.components,
                weighted_total: score.weighted_total,
                score: score.score,
            })
            .collect()
    })
}
