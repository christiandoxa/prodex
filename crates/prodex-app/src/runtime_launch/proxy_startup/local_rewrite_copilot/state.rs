//! Copilot provider auth state, profile catalog, and OAuth pool construction.

use super::super::local_rewrite::RuntimeLocalRewriteProviderOptions;
use super::super::local_rewrite_copilot_bindings::RuntimeCopilotBindingRecorder;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) enum RuntimeCopilotProviderAuth {
    ApiKeys {
        api_keys: Vec<String>,
    },
    Profiles {
        profiles: Vec<RuntimeCopilotProfileAuth>,
    },
}

#[derive(Clone)]
pub(crate) struct RuntimeCopilotProfileAuth {
    pub(crate) profile_name: String,
    pub(crate) api_key: String,
    pub(crate) api_url: String,
    pub(crate) model_catalog: Vec<serde_json::Value>,
}

impl fmt::Debug for RuntimeCopilotProfileAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeCopilotProfileAuth")
            .field("profile_name", &"<redacted>")
            .field("api_key", &"<redacted>")
            .field("api_url", &"<redacted>")
            .field("model_catalog", &redacted_len(self.model_catalog.len()))
            .finish()
    }
}

#[derive(Clone)]
pub(in crate::runtime_launch::proxy_startup) struct RuntimeCopilotOAuthPool {
    pub(super) state: Arc<Mutex<RuntimeCopilotOAuthPoolState>>,
}

pub(super) struct RuntimeCopilotOAuthPoolState {
    pub(super) profiles: Vec<RuntimeCopilotProfileAuth>,
    pub(super) next_index: usize,
    pub(super) response_profile_bindings: BTreeMap<String, String>,
}

impl fmt::Debug for RuntimeCopilotOAuthPoolState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeCopilotOAuthPoolState")
            .field("profiles", &redacted_len(self.profiles.len()))
            .field("next_index", &"<redacted>")
            .field(
                "response_profile_bindings",
                &redacted_len(self.response_profile_bindings.len()),
            )
            .finish()
    }
}

fn redacted_len(len: usize) -> String {
    format!("<redacted:{len}>")
}

#[derive(Clone)]
pub(super) struct RuntimeCopilotSelectedAuth {
    pub(super) profile_name: String,
    pub(super) api_key: String,
    pub(super) api_url: Option<String>,
    pub(super) hard_affinity: bool,
}

#[derive(Clone)]
pub(in crate::runtime_launch::proxy_startup) struct RuntimeCopilotRequestContext {
    pub(in crate::runtime_launch::proxy_startup) profile_name: String,
    pub(in crate::runtime_launch::proxy_startup) binding_recorder:
        Option<RuntimeCopilotBindingRecorder>,
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_copilot_model_catalog_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Vec<serde_json::Value> {
    let RuntimeLocalRewriteProviderOptions::Copilot {
        auth: RuntimeCopilotProviderAuth::Profiles { profiles },
    } = provider
    else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut catalog = Vec::new();
    for profile in profiles {
        for model in &profile.model_catalog {
            let Some(id) = model.get("id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let id = id.trim();
            if id.is_empty() || !seen.insert(id.to_ascii_lowercase()) {
                continue;
            }
            catalog.push(model.clone());
        }
    }
    catalog
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_copilot_oauth_pool_from_provider(
    provider: &RuntimeLocalRewriteProviderOptions,
) -> Option<RuntimeCopilotOAuthPool> {
    let RuntimeLocalRewriteProviderOptions::Copilot {
        auth: RuntimeCopilotProviderAuth::Profiles { profiles },
    } = provider
    else {
        return None;
    };
    Some(RuntimeCopilotOAuthPool {
        state: Arc::new(Mutex::new(RuntimeCopilotOAuthPoolState {
            profiles: profiles.clone(),
            next_index: 0,
            response_profile_bindings: BTreeMap::new(),
        })),
    })
}
