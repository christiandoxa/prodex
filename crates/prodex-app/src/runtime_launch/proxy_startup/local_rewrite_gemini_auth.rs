use super::{RuntimeGeminiOAuthPool, RuntimeGeminiSelectedAuth};
use crate::{GeminiOAuthSecret, force_refresh_gemini_oauth_secret};
use anyhow::Result;

impl RuntimeGeminiOAuthPool {
    pub(super) fn refresh_profile_auth(
        &self,
        profile_name: &str,
        hard_affinity: bool,
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
        self.remember_refreshed_auth(profile_name, secret, hard_affinity)
    }

    pub(super) fn remember_refreshed_auth(
        &self,
        profile_name: &str,
        secret: GeminiOAuthSecret,
        hard_affinity: bool,
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
        }))
    }
}
