use super::*;
use crate::smart_context::{
    smart_context_recent_rewrite_min_saved_tokens,
    smart_context_rewrite_telemetry_average_body_ratio_percent,
    smart_context_rewrite_telemetry_sample_safe_saved,
    smart_context_rewrite_telemetry_saved_tokens,
    smart_context_transform_rewrite_safety_reasons_for_decision,
    smart_context_transform_rewrite_safety_score_empty,
    smart_context_transform_rewrite_safety_score_value,
};
use std::collections::{BTreeMap, BTreeSet};

pub fn smart_context_transform_rewrite_budget_decision(
    input: SmartContextTransformRewriteSafetyScoreInput,
) -> SmartContextRewriteBudgetDecision {
    smart_context_transform_rewrite_safety_score(input).decision
}

pub fn smart_context_transform_rewrite_safety_score(
    input: SmartContextTransformRewriteSafetyScoreInput,
) -> SmartContextTransformRewriteSafetyScore {
    let SmartContextTransformRewriteSafetyScoreInput {
        category,
        recent_rewrite_safety,
        telemetry_samples,
    } = input;
    let recent = telemetry_samples
        .iter()
        .filter(|sample| sample.category == category)
        .map(|sample| sample.sample)
        .rev()
        .take(SMART_CONTEXT_REWRITE_TELEMETRY_RECENT_LIMIT)
        .collect::<Vec<_>>();
    let safety_samples = recent_rewrite_safety
        .safe_rewrites
        .saturating_add(recent_rewrite_safety.fallback_rewrites);

    if recent.is_empty() {
        let mut score =
            smart_context_transform_rewrite_safety_score_empty(category, &recent_rewrite_safety);
        score.safety_samples = safety_samples;
        return score;
    }

    let safe_samples = recent
        .iter()
        .filter(|sample| smart_context_rewrite_telemetry_sample_safe_saved(sample))
        .count();
    let fallback_samples = recent.iter().filter(|sample| sample.fallback).count();
    let unsafe_samples = recent.iter().filter(|sample| !sample.safe).count();
    let weak_savings_samples = recent
        .iter()
        .filter(|sample| {
            !sample.fallback
                && sample.safe
                && !smart_context_rewrite_telemetry_sample_safe_saved(sample)
        })
        .count();
    let saved_tokens = smart_context_rewrite_telemetry_saved_tokens(&recent);
    let average_body_ratio_percent_value =
        smart_context_rewrite_telemetry_average_body_ratio_percent(&recent);
    let average_body_ratio_percent = Some(average_body_ratio_percent_value);
    let mut reasons = Vec::new();

    if fallback_samples > 0 || unsafe_samples > 0 || weak_savings_samples > 0 {
        if fallback_samples > 0 {
            reasons.push(SmartContextTransformRewriteSafetyReason::FallbackObserved);
        }
        if unsafe_samples > 0 {
            reasons.push(SmartContextTransformRewriteSafetyReason::UnsafeSample);
        }
        if weak_savings_samples > 0 {
            reasons.push(SmartContextTransformRewriteSafetyReason::WeakSavings);
        }
        return SmartContextTransformRewriteSafetyScore {
            category,
            decision: SmartContextRewriteBudgetDecision::Tighten,
            safety_score: -100,
            telemetry_samples: recent.len(),
            safety_samples,
            safe_samples,
            fallback_samples,
            unsafe_samples,
            weak_savings_samples,
            saved_tokens,
            average_body_ratio_percent,
            reasons,
        };
    }

    if recent.len() < SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT {
        let decision = smart_context_recent_rewrite_safety_budget_decision(&recent_rewrite_safety);
        reasons.push(SmartContextTransformRewriteSafetyReason::InsufficientTransformSamples);
        if safety_samples > 0 {
            reasons.push(SmartContextTransformRewriteSafetyReason::RecentSafetyFallback);
            reasons.extend(smart_context_transform_rewrite_safety_reasons_for_decision(
                decision,
            ));
        }
        return SmartContextTransformRewriteSafetyScore {
            category,
            decision,
            safety_score: smart_context_transform_rewrite_safety_score_value(decision),
            telemetry_samples: recent.len(),
            safety_samples,
            safe_samples,
            fallback_samples,
            unsafe_samples,
            weak_savings_samples,
            saved_tokens,
            average_body_ratio_percent,
            reasons,
        };
    }

    let required_saved_tokens = smart_context_recent_rewrite_min_saved_tokens(recent.len());
    let decision = if saved_tokens >= required_saved_tokens
        && average_body_ratio_percent_value
            <= SMART_CONTEXT_REWRITE_TELEMETRY_RELAX_MAX_AVERAGE_BODY_RATIO_PERCENT
    {
        SmartContextRewriteBudgetDecision::Relax
    } else if saved_tokens < required_saved_tokens
        || average_body_ratio_percent_value
            >= SMART_CONTEXT_REWRITE_TELEMETRY_TIGHTEN_MIN_AVERAGE_BODY_RATIO_PERCENT
    {
        SmartContextRewriteBudgetDecision::Tighten
    } else {
        SmartContextRewriteBudgetDecision::NoChange
    };
    reasons.extend(smart_context_transform_rewrite_safety_reasons_for_decision(
        decision,
    ));

    SmartContextTransformRewriteSafetyScore {
        category,
        decision,
        safety_score: smart_context_transform_rewrite_safety_score_value(decision),
        telemetry_samples: recent.len(),
        safety_samples,
        safe_samples,
        fallback_samples,
        unsafe_samples,
        weak_savings_samples,
        saved_tokens,
        average_body_ratio_percent,
        reasons,
    }
}

pub fn smart_context_per_transform_rewrite_safety_scores(
    input: SmartContextPerTransformRewriteSafetyInput,
) -> Vec<SmartContextTransformRewriteSafetyScore> {
    let mut categories = BTreeSet::new();
    let mut recent_safety =
        BTreeMap::<SmartContextTransformCategory, SmartContextRecentRewriteSafety>::new();

    for item in input.recent_rewrite_safety {
        categories.insert(item.category.clone());
        recent_safety
            .entry(item.category)
            .and_modify(|existing| {
                existing.safe_rewrites = existing
                    .safe_rewrites
                    .saturating_add(item.safety.safe_rewrites);
                existing.fallback_rewrites = existing
                    .fallback_rewrites
                    .saturating_add(item.safety.fallback_rewrites);
                existing.saved_tokens = existing
                    .saved_tokens
                    .saturating_add(item.safety.saved_tokens);
            })
            .or_insert(item.safety);
    }
    for sample in &input.telemetry_samples {
        categories.insert(sample.category.clone());
    }

    categories
        .into_iter()
        .map(|category| {
            smart_context_transform_rewrite_safety_score(
                SmartContextTransformRewriteSafetyScoreInput {
                    recent_rewrite_safety: recent_safety
                        .get(&category)
                        .copied()
                        .unwrap_or_default(),
                    telemetry_samples: input.telemetry_samples.clone(),
                    category,
                },
            )
        })
        .collect()
}
