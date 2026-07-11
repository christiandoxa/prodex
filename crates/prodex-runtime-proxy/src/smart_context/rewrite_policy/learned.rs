use super::*;
use crate::smart_context::{
    smart_context_learned_rewrite_policy_exact, smart_context_rewrite_policy_bucket_key_complete,
    smart_context_rewrite_policy_bucket_key_matches,
    smart_context_rewrite_telemetry_sample_safe_saved,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SmartContextLearnedRewritePolicyReason {
    BasePolicyExactPassThrough,
    MissingBucketKey,
    MissingBucketConfidence,
    UnsafeRewriteSample,
    RecentSafetyFallback,
    LearnedNoChange,
    LearnedRelax,
    LearnedTighten,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextLearnedRewritePolicyInput {
    pub adaptive_policy_input: SmartContextAdaptiveBudgetPolicyInput,
    pub bucket_key: Option<SmartContextRewritePolicyBucketKey>,
    pub telemetry_samples: Vec<SmartContextBucketedRewriteTelemetrySample>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextLearnedRewritePolicy {
    pub policy: SmartContextAdaptiveBudgetPolicy,
    pub decision: SmartContextRewriteBudgetDecision,
    pub reasons: Vec<SmartContextLearnedRewritePolicyReason>,
    pub matching_telemetry_samples: usize,
    pub safety_samples: usize,
}

pub fn smart_context_learned_rewrite_policy(
    input: SmartContextLearnedRewritePolicyInput,
) -> SmartContextLearnedRewritePolicy {
    let SmartContextLearnedRewritePolicyInput {
        mut adaptive_policy_input,
        bucket_key,
        telemetry_samples,
    } = input;
    let recent_rewrite_safety = adaptive_policy_input.recent_rewrite_safety;
    let safety_samples = recent_rewrite_safety
        .safe_rewrites
        .saturating_add(recent_rewrite_safety.fallback_rewrites);
    let available_context_tokens = adaptive_policy_input.accounting.available_context_tokens;

    adaptive_policy_input.recent_rewrite_safety = SmartContextRecentRewriteSafety::default();
    let base_policy = smart_context_adaptive_budget_policy(adaptive_policy_input);
    if base_policy.mode == SmartContextBudgetMode::ExactPassThrough {
        return SmartContextLearnedRewritePolicy {
            policy: base_policy,
            decision: SmartContextRewriteBudgetDecision::NoChange,
            reasons: vec![SmartContextLearnedRewritePolicyReason::BasePolicyExactPassThrough],
            matching_telemetry_samples: 0,
            safety_samples,
        };
    }

    let Some(bucket_key) = bucket_key
        .as_ref()
        .filter(|bucket_key| smart_context_rewrite_policy_bucket_key_complete(bucket_key))
    else {
        return smart_context_learned_rewrite_policy_exact(
            base_policy,
            SmartContextLearnedRewritePolicyReason::MissingBucketKey,
            0,
            safety_samples,
            available_context_tokens,
        );
    };

    let matching_telemetry_samples = telemetry_samples
        .into_iter()
        .filter(|sample| {
            smart_context_rewrite_policy_bucket_key_matches(bucket_key, sample.bucket_key.as_ref())
        })
        .map(|sample| sample.sample)
        .collect::<Vec<_>>();
    let matching_sample_count = matching_telemetry_samples.len();
    let telemetry_confident =
        matching_sample_count >= SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT;
    let safety_confident = matching_sample_count == 0
        && safety_samples >= SMART_CONTEXT_REWRITE_TELEMETRY_MIN_SAMPLE_COUNT;

    if matching_telemetry_samples
        .iter()
        .any(|sample| sample.fallback || !smart_context_rewrite_telemetry_sample_safe_saved(sample))
        || recent_rewrite_safety.fallback_rewrites > 0
    {
        return smart_context_learned_rewrite_policy_exact(
            base_policy,
            SmartContextLearnedRewritePolicyReason::UnsafeRewriteSample,
            matching_sample_count,
            safety_samples,
            available_context_tokens,
        );
    }
    if !telemetry_confident && !safety_confident {
        return smart_context_learned_rewrite_policy_exact(
            base_policy,
            SmartContextLearnedRewritePolicyReason::MissingBucketConfidence,
            matching_sample_count,
            safety_samples,
            available_context_tokens,
        );
    }

    let decision = if telemetry_confident {
        smart_context_rewrite_telemetry_budget_decision(SmartContextRewriteTelemetryBudgetInput {
            recent_rewrite_safety,
            telemetry_samples: matching_telemetry_samples,
        })
    } else {
        smart_context_recent_rewrite_safety_budget_decision(&recent_rewrite_safety)
    };
    let policy = smart_context_apply_rewrite_budget_decision(
        base_policy,
        decision,
        available_context_tokens,
    );
    let mut reasons = Vec::new();
    if safety_confident {
        reasons.push(SmartContextLearnedRewritePolicyReason::RecentSafetyFallback);
    }
    reasons.push(match decision {
        SmartContextRewriteBudgetDecision::NoChange => {
            SmartContextLearnedRewritePolicyReason::LearnedNoChange
        }
        SmartContextRewriteBudgetDecision::Relax => {
            SmartContextLearnedRewritePolicyReason::LearnedRelax
        }
        SmartContextRewriteBudgetDecision::Tighten => {
            SmartContextLearnedRewritePolicyReason::LearnedTighten
        }
    });

    SmartContextLearnedRewritePolicy {
        policy,
        decision,
        reasons,
        matching_telemetry_samples: matching_sample_count,
        safety_samples,
    }
}
