use super::{RuntimeGeminiOAuthPool, RuntimeGeminiOAuthProfileAuth, RuntimeGeminiSelectedAuth};
use crate::{GeminiOAuthSecret, force_refresh_gemini_oauth_secret};
use anyhow::Result;

impl RuntimeGeminiOAuthPool {
    pub(super) fn refresh_profile_auth(
        &self,
        profile_name: &str,
        hard_affinity: bool,
        quota_fallback_allowed: bool,
    ) -> Result<Option<RuntimeGeminiSelectedAuth>> {
        let codex_home = {
            let state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("Gemini OAuth pool lock poisoned"))?;
            let Some(profile) = state.profile_by_name(profile_name) else {
                return Ok(None);
            };
            profile.codex_home
        };
        let secret = force_refresh_gemini_oauth_secret(&codex_home)?;
        self.remember_refreshed_auth(profile_name, secret, hard_affinity, quota_fallback_allowed)
    }

    pub(super) fn remember_refreshed_auth(
        &self,
        profile_name: &str,
        secret: GeminiOAuthSecret,
        hard_affinity: bool,
        quota_fallback_allowed: bool,
    ) -> Result<Option<RuntimeGeminiSelectedAuth>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Gemini OAuth pool lock poisoned"))?;
        let Some(profile) = state
            .profiles
            .iter_mut()
            .find(|profile| profile.profile_name == profile_name)
        else {
            return Ok(None);
        };
        profile.access_token = secret.access_token;
        profile.email = Some(secret.email);
        if secret.project_id.is_some() {
            profile.project_id = secret.project_id;
        }
        Ok(Some(RuntimeGeminiSelectedAuth {
            profile_name: profile.profile_name.clone(),
            auth: profile.auth(),
            hard_affinity,
            quota_fallback_allowed,
        }))
    }
}

pub(super) fn runtime_gemini_oauth_affinity_attempts(
    profiles: &[RuntimeGeminiOAuthProfileAuth],
    profile_name: &str,
) -> Option<Vec<RuntimeGeminiSelectedAuth>> {
    let profile = profiles
        .iter()
        .find(|profile| profile.profile_name == profile_name)?;
    let mut attempts = vec![RuntimeGeminiSelectedAuth {
        profile_name: profile.profile_name.clone(),
        auth: profile.auth(),
        hard_affinity: true,
        quota_fallback_allowed: profiles.len() > 1,
    }];
    attempts.extend(
        profiles
            .iter()
            .filter(|candidate| candidate.profile_name != profile.profile_name)
            .map(runtime_gemini_oauth_attempt_from_profile),
    );
    Some(attempts)
}

pub(super) fn runtime_gemini_oauth_attempt_from_profile(
    profile: &RuntimeGeminiOAuthProfileAuth,
) -> RuntimeGeminiSelectedAuth {
    RuntimeGeminiSelectedAuth {
        profile_name: profile.profile_name.clone(),
        auth: profile.auth(),
        hard_affinity: false,
        quota_fallback_allowed: false,
    }
}
