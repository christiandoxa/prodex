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

pub fn smart_context_rollout_bucket(stable_key: &str) -> u16 {
    let digest = smart_context_sha256_digest(stable_key.as_bytes());
    let prefix = u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    (prefix % 10_000) as u16
}
