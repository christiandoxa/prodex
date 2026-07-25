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
    pub fn applies_rewrite(&self) -> bool {
        self.mode == SmartContextRolloutMode::Apply
    }

    pub fn computes_shadow(&self) -> bool {
        self.mode == SmartContextRolloutMode::Shadow
    }
}

pub fn smart_context_rollout_decision(
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
