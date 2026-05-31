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

#[derive(Clone, Debug)]
pub(crate) struct RuntimeAnthropicOAuthProfileAuth {
    pub(crate) access_token: String,
}

impl RuntimeAnthropicOAuthProfileAuth {
    pub(crate) fn auth(&self) -> RuntimeAnthropicAuth {
        RuntimeAnthropicAuth::OAuth {
            access_token: self.access_token.clone(),
        }
    }
}
