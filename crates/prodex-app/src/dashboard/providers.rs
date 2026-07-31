use anyhow::Result;
use prodex_provider_core::{ProviderId, provider_model_catalog_json};
use serde_json::{Value, json};

use crate::{AppState, ProfileEntry, ProfileProvider};

pub(super) const DASHBOARD_PROVIDER_IDS: &[ProviderId] = &[
    ProviderId::OpenAi,
    ProviderId::Gemini,
    ProviderId::Anthropic,
    ProviderId::Copilot,
    ProviderId::DeepSeek,
    ProviderId::Kiro,
    ProviderId::Local,
];

pub(super) fn provider_presets(state: &AppState) -> Result<Vec<Value>> {
    DASHBOARD_PROVIDER_IDS
        .iter()
        .copied()
        .map(|provider| -> Result<Value> {
            let configured_profiles = provider_profile_count(state, provider)?;
            let default_model = provider_default_model(provider);
            Ok(json!({
                "id": provider.label(),
                "label": provider_display_name(provider),
                "auth": provider_auth_summary(provider),
                "defaultModel": default_model,
                "recommendedModel": default_model,
                "modelCount": provider_model_catalog_json(provider).len(),
                "configuredProfiles": configured_profiles,
                "active": provider_has_active_profile(state, provider)?,
                "availableThrough": provider_available_through(provider, configured_profiles),
                "commands": {
                    "setup": provider_setup_commands(provider),
                    "launch": provider_launch_command(provider, None),
                    "quota": provider_quota_command(provider),
                    "gateway": provider_gateway_command(provider),
                },
                "notes": provider_notes(provider),
            }))
        })
        .collect()
}

pub(super) fn provider_profile_count(state: &AppState, provider: ProviderId) -> Result<usize> {
    state.profiles.values().try_fold(0, |count, profile| {
        Ok(count + usize::from(profile_catalog_provider(profile)? == Some(provider)))
    })
}

fn provider_has_active_profile(state: &AppState, provider: ProviderId) -> Result<bool> {
    Ok(state
        .active_profile
        .as_ref()
        .and_then(|name| state.profiles.get(name))
        .map(profile_catalog_provider)
        .transpose()?
        .flatten()
        == Some(provider))
}

fn profile_catalog_provider(profile: &ProfileEntry) -> Result<Option<ProviderId>> {
    Ok(match &profile.provider {
        ProfileProvider::Openai => {
            let model_provider = crate::codex_non_openai_model_provider(&profile.codex_home, None)?;
            match model_provider
                .as_ref()
                .map(|provider| provider.provider_id.as_str())
            {
                Some(id) if id.eq_ignore_ascii_case(crate::SUPER_DEEPSEEK_PROVIDER_ID) => {
                    Some(ProviderId::DeepSeek)
                }
                Some(id) if id.eq_ignore_ascii_case(crate::SUPER_LOCAL_PROVIDER_ID) => {
                    Some(ProviderId::Local)
                }
                _ => Some(ProviderId::OpenAi),
            }
        }
        ProfileProvider::Gemini { .. } => Some(ProviderId::Gemini),
        ProfileProvider::Anthropic { .. } => Some(ProviderId::Anthropic),
        ProfileProvider::Copilot { .. } => Some(ProviderId::Copilot),
        ProfileProvider::Kiro { .. } => Some(ProviderId::Kiro),
        ProfileProvider::Agy { .. } => None,
    })
}

pub(super) fn provider_display_name(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "OpenAI / ChatGPT Codex",
        ProviderId::Gemini => "Google Gemini",
        ProviderId::Anthropic => "Anthropic Claude",
        ProviderId::Copilot => "GitHub Copilot",
        ProviderId::DeepSeek => "DeepSeek",
        ProviderId::Kiro => "Kiro CLI",
        ProviderId::Local => "Local OpenAI-compatible",
    }
}

fn provider_auth_summary(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "ChatGPT login, device code, or API key profile",
        ProviderId::Gemini => "Google OAuth profile or GEMINI_API_KEY(S)",
        ProviderId::Anthropic => "Claude OAuth import or ANTHROPIC_API_KEY(S)",
        ProviderId::Copilot => "Copilot CLI import or GITHUB_COPILOT_API_KEY(S)",
        ProviderId::DeepSeek => "DEEPSEEK_API_KEY(S)",
        ProviderId::Kiro => "Kiro CLI import",
        ProviderId::Local => "Local base URL; API key optional",
    }
}

pub(super) fn provider_default_model(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "gpt-5.3-codex",
        ProviderId::Gemini => crate::SUPER_GEMINI_DEFAULT_MODEL,
        ProviderId::Anthropic => crate::SUPER_ANTHROPIC_DEFAULT_MODEL,
        ProviderId::Copilot => crate::SUPER_COPILOT_DEFAULT_MODEL,
        ProviderId::DeepSeek => crate::SUPER_DEEPSEEK_DEFAULT_MODEL,
        ProviderId::Kiro => crate::SUPER_KIRO_DEFAULT_MODEL,
        ProviderId::Local => crate::SUPER_DEFAULT_LOCAL_MODEL,
    }
}

pub(super) fn provider_available_through(
    provider: ProviderId,
    configured_profiles: usize,
) -> Vec<&'static str> {
    let mut routes = Vec::new();
    if configured_profiles > 0 {
        routes.push("profile-backed routing");
    }
    match provider {
        ProviderId::OpenAi => routes.push("gateway"),
        ProviderId::Gemini
        | ProviderId::Anthropic
        | ProviderId::Copilot
        | ProviderId::DeepSeek
        | ProviderId::Kiro => {
            routes.push("runtime provider launch");
            routes.push("gateway");
        }
        ProviderId::Local => {
            routes.push("local URL");
            routes.push("gateway");
        }
    }
    routes
}

fn provider_setup_commands(provider: ProviderId) -> Vec<&'static str> {
    match provider {
        ProviderId::OpenAi => vec![
            "prodex profile add openai-main --activate",
            "prodex login --profile openai-main",
            "prodex profile import-current openai-main",
        ],
        ProviderId::Gemini => vec![
            "prodex login --with-google",
            "GEMINI_API_KEY=... prodex s gemini --model auto",
        ],
        ProviderId::Anthropic => vec![
            "prodex login --with-claude",
            "prodex profile import claude --activate",
            "ANTHROPIC_API_KEY=... prodex s --provider anthropic --model claude-sonnet-4-6",
        ],
        ProviderId::Copilot => vec![
            "prodex profile import copilot --activate",
            "GITHUB_COPILOT_API_KEY=... prodex s --provider copilot --model gpt-5.3-codex",
        ],
        ProviderId::DeepSeek => {
            vec!["DEEPSEEK_API_KEY=... prodex s deepseek --model deepseek-v4-pro"]
        }
        ProviderId::Kiro => vec![
            "prodex profile import kiro --activate",
            "prodex s --provider kiro --model claude-sonnet-4",
        ],
        ProviderId::Local => {
            vec!["prodex super --url http://127.0.0.1:8131 --model unsloth/qwen3.5-35b-a3b"]
        }
    }
}

pub(super) fn provider_launch_command(provider: ProviderId, model: Option<&str>) -> String {
    let model = model.unwrap_or_else(|| provider_default_model(provider));
    match provider {
        ProviderId::OpenAi => format!("prodex s -m {model}"),
        ProviderId::Gemini => format!("prodex s gemini --model {model}"),
        ProviderId::Anthropic => format!("prodex s --provider anthropic --model {model}"),
        ProviderId::Copilot => format!("prodex s --provider copilot --model {model}"),
        ProviderId::DeepSeek => format!("prodex s deepseek --model {model}"),
        ProviderId::Kiro => format!("prodex s --provider kiro --model {model}"),
        ProviderId::Local => format!("prodex super --url http://127.0.0.1:8131 --model {model}"),
    }
}

fn provider_quota_command(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "prodex quota --all --provider openai --once",
        ProviderId::Gemini => "prodex quota --all --provider gemini --once",
        ProviderId::Anthropic => "prodex quota --all --provider anthropic --once",
        ProviderId::Copilot => "prodex quota --all --provider copilot --once",
        ProviderId::DeepSeek => "prodex quota --all --provider deepseek --once",
        ProviderId::Kiro => "prodex quota --all --provider kiro --once",
        ProviderId::Local => {
            "prodex quota --all --provider local --base-url http://127.0.0.1:8131/v1 --once"
        }
    }
}

fn provider_gateway_command(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => "prodex gateway",
        ProviderId::Gemini => "prodex gateway --provider gemini",
        ProviderId::Anthropic => "prodex gateway --provider anthropic",
        ProviderId::Copilot => "prodex gateway --provider copilot",
        ProviderId::DeepSeek => "prodex gateway --provider deepseek",
        ProviderId::Kiro => "prodex gateway --provider kiro",
        ProviderId::Local => "prodex gateway --base-url http://127.0.0.1:8131/v1",
    }
}

fn provider_notes(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::OpenAi => {
            "Prodex profile pool keeps quota-aware rotation and continuation affinity."
        }
        ProviderId::Gemini => {
            "OAuth profiles use Code Assist; API-key launches use the Gemini OpenAI-compatible endpoint."
        }
        ProviderId::Anthropic => {
            "Command generation only; dashboard does not store Anthropic secrets."
        }
        ProviderId::Copilot => {
            "Imported profiles keep Copilot credentials in Copilot-owned storage."
        }
        ProviderId::DeepSeek => {
            "API-key runtime bridge; no profile secret is stored by this dashboard."
        }
        ProviderId::Kiro => "Import snapshots Kiro CLI auth for Prodex routing.",
        ProviderId::Local => "Point Prodex at a local OpenAI-compatible /v1 server.",
    }
}
