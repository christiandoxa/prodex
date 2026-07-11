use std::fmt;

#[derive(Clone)]
pub(crate) enum RuntimeAnthropicAuth {
    ApiKey { api_key: String },
    OAuth { access_token: String },
}

#[derive(Clone)]
pub(crate) enum RuntimeAnthropicProviderAuth {
    ApiKeys {
        api_keys: Vec<String>,
    },
    OAuthProfiles {
        profiles: Vec<RuntimeAnthropicOAuthProfileAuth>,
    },
}

#[derive(Clone)]
pub(crate) struct RuntimeAnthropicOAuthProfileAuth {
    pub(crate) profile_name: String,
    pub(crate) access_token: String,
}

impl fmt::Debug for RuntimeAnthropicOAuthProfileAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeAnthropicOAuthProfileAuth")
            .field("profile_name", &"<redacted>")
            .field("access_token", &"<redacted>")
            .finish()
    }
}

impl RuntimeAnthropicOAuthProfileAuth {
    pub(crate) fn auth(&self) -> RuntimeAnthropicAuth {
        RuntimeAnthropicAuth::OAuth {
            access_token: self.access_token.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_oauth_profile_debug_output_redacts_sensitive_fields() {
        let profile = RuntimeAnthropicOAuthProfileAuth {
            profile_name: "anthropic-profile-secret".to_string(),
            access_token: "anthropic-access-token-secret".to_string(),
        };
        let rendered = format!("{profile:?}");

        assert!(rendered.contains("RuntimeAnthropicOAuthProfileAuth"));
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains("anthropic-profile-secret"), "{rendered}");
        assert!(
            !rendered.contains("anthropic-access-token-secret"),
            "{rendered}"
        );
    }
}
