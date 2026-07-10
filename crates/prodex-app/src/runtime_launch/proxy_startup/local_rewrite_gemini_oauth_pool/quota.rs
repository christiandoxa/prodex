//! Gemini OAuth quota refresh worker and cached quota headers.

use super::super::super::local_rewrite_rate_limits::runtime_gemini_quota_codex_headers;
use super::RuntimeGeminiOAuthPool;
use crate::{
    fetch_gemini_quota_with_code_assist_endpoint, gemini_code_assist_endpoint,
    runtime_proxy_log_to_path, spawn_runtime_background_worker_or_log,
};
use runtime_proxy_crate::{runtime_proxy_log_field, runtime_proxy_structured_log_message};
use std::path::PathBuf;

impl RuntimeGeminiOAuthPool {
    pub(in crate::runtime_launch::proxy_startup) fn spawn_quota_refresh(&self, log_path: PathBuf) {
        let profiles = match self.state.lock() {
            Ok(state) => state.profiles.clone(),
            Err(_) => return,
        };
        if profiles.is_empty() {
            return;
        }
        let pool = self.clone();
        let code_assist_endpoint = gemini_code_assist_endpoint();
        spawn_runtime_background_worker_or_log(
            "prodex-gemini-quota-refresh",
            Some(log_path.clone()),
            move || {
                for profile in profiles {
                    let result = fetch_gemini_quota_with_code_assist_endpoint(
                        &profile.codex_home,
                        profile.project_id.as_deref(),
                        &code_assist_endpoint,
                    );
                    match result {
                        Ok(info) => {
                            let headers = runtime_gemini_quota_codex_headers(
                                &profile.profile_name,
                                profile.email.as_deref(),
                                &info,
                            );
                            pool.remember_quota_headers(&profile.profile_name, headers);
                            runtime_proxy_log_to_path(
                                &log_path,
                                &runtime_proxy_structured_log_message(
                                    "local_rewrite_gemini_quota_status_ready",
                                    [
                                        runtime_proxy_log_field(
                                            "profile",
                                            profile.profile_name.as_str(),
                                        ),
                                        runtime_proxy_log_field(
                                            "buckets",
                                            info.buckets.len().to_string(),
                                        ),
                                    ],
                                ),
                            );
                        }
                        Err(err) => {
                            runtime_proxy_log_to_path(
                                &log_path,
                                &runtime_proxy_structured_log_message(
                                    "local_rewrite_gemini_quota_status_unavailable",
                                    [
                                        runtime_proxy_log_field(
                                            "profile",
                                            profile.profile_name.as_str(),
                                        ),
                                        runtime_proxy_log_field("error", err.to_string()),
                                    ],
                                ),
                            );
                        }
                    }
                }
            },
        );
    }

    pub(in crate::runtime_launch::proxy_startup) fn quota_headers_for_profile(
        &self,
        profile_name: &str,
    ) -> Vec<(String, String)> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.quota_headers.get(profile_name).cloned())
            .unwrap_or_default()
    }

    fn remember_quota_headers(&self, profile_name: &str, headers: Vec<(String, String)>) {
        if headers.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            state
                .quota_headers
                .insert(profile_name.to_string(), headers);
        }
    }
}
