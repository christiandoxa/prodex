use crate::smart_context::smart_context_sha256_digest;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SmartContextRolloutMode {
    #[default]
    Apply,
    Shadow,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRolloutDecisionInput {
    pub enabled: bool,
    pub explicit_exact_mode: bool,
    pub shadow_mode: bool,
    pub canary_percent: u8,
    pub stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextRolloutDecision {
    pub mode: SmartContextRolloutMode,
    pub canary_bucket: u16,
    pub canary_percent: u8,
    pub reason: &'static str,
}

impl SmartContextRolloutDecision {
    #[cfg(test)]
    pub(crate) fn applies_rewrite(&self) -> bool {
        self.mode == SmartContextRolloutMode::Apply
    }

    pub fn computes_shadow(&self) -> bool {
        self.mode == SmartContextRolloutMode::Shadow
    }
}

pub fn smart_context_rollout_decision(
    input: SmartContextRolloutDecisionInput,
) -> SmartContextRolloutDecision {
    #[cfg(feature = "mojo")]
    {
        let canary_bucket = smart_context_rollout_bucket(&input.stable_key);
        let plan = prodex_mojo_core::runtime::smart_context_rollout_plan(
            input.enabled,
            input.explicit_exact_mode,
            input.shadow_mode,
            input.canary_percent,
            canary_bucket,
        )
        .expect("Mojo Smart Context rollout plan returned invalid output");
        SmartContextRolloutDecision {
            mode: match plan.mode {
                0 => SmartContextRolloutMode::Apply,
                1 => SmartContextRolloutMode::Shadow,
                2 => SmartContextRolloutMode::Disabled,
                _ => unreachable!("validated by prodex-mojo-core"),
            },
            canary_bucket,
            canary_percent: plan.canary_percent,
            reason: match plan.reason {
                0 => "disabled",
                1 => "explicit_exact",
                2 => "shadow",
                3 => "canary_out",
                4 => "enabled",
                5 => "canary_in",
                _ => unreachable!("validated by prodex-mojo-core"),
            },
        }
    }

    #[cfg(not(feature = "mojo"))]
    smart_context_rollout_decision_rust(input)
}

#[cfg(any(not(feature = "mojo"), test))]
pub(in crate::smart_context) fn smart_context_rollout_decision_rust(
    input: SmartContextRolloutDecisionInput,
) -> SmartContextRolloutDecision {
    let canary_percent = input.canary_percent.min(100);
    let canary_bucket = smart_context_rollout_bucket(&input.stable_key);
    if !input.enabled {
        return SmartContextRolloutDecision {
            mode: SmartContextRolloutMode::Disabled,
            canary_bucket,
            canary_percent,
            reason: "disabled",
        };
    }
    if input.explicit_exact_mode {
        return SmartContextRolloutDecision {
            mode: SmartContextRolloutMode::Disabled,
            canary_bucket,
            canary_percent,
            reason: "explicit_exact",
        };
    }
    if input.shadow_mode {
        return SmartContextRolloutDecision {
            mode: SmartContextRolloutMode::Shadow,
            canary_bucket,
            canary_percent,
            reason: "shadow",
        };
    }
    if canary_percent < 100 && canary_bucket >= u16::from(canary_percent) * 100 {
        return SmartContextRolloutDecision {
            mode: SmartContextRolloutMode::Disabled,
            canary_bucket,
            canary_percent,
            reason: "canary_out",
        };
    }
    SmartContextRolloutDecision {
        mode: SmartContextRolloutMode::Apply,
        canary_bucket,
        canary_percent,
        reason: if canary_percent == 100 {
            "enabled"
        } else {
            "canary_in"
        },
    }
}

pub fn smart_context_rollout_bucket(stable_key: &str) -> u16 {
    let digest = smart_context_sha256_digest(stable_key.as_bytes());
    let prefix = u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    (prefix % 10_000) as u16
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    #[test]
    fn rollout_plan_matches_rust_oracle() {
        for mask in 0..32_u8 {
            for canary_percent in [0, 1, 50, 99, 100, 101, u8::MAX] {
                let input = SmartContextRolloutDecisionInput {
                    enabled: mask & 1 != 0,
                    explicit_exact_mode: mask & 2 != 0,
                    shadow_mode: mask & 4 != 0,
                    canary_percent,
                    stable_key: format!("rollout-{mask}-{canary_percent}"),
                };
                assert_eq!(
                    smart_context_rollout_decision(input.clone()),
                    smart_context_rollout_decision_rust(input)
                );
            }
        }
    }
}
