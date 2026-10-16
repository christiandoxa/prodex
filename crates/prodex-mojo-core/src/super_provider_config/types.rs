use crate::MojoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderAliasPlan {
    Anthropic,
    Copilot,
    DeepSeek,
    Gemini,
    Kiro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeExternalProviderClass {
    Anthropic,
    Copilot,
    DeepSeek,
    Gemini,
    GeminiOauth,
    Kiro,
    GeminiNative,
    Antigravity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileImportSourceClass {
    Claude,
    Copilot,
    Kiro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeModelProviderClass {
    Local,
    DeepSeek,
    Gemini,
    Anthropic,
    Copilot,
    Kiro,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalCatalogProviderClass {
    Anthropic = 0,
    Copilot = 1,
    Kiro = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCatalogStaticModel {
    pub slug: String,
    pub display_name: String,
    pub description: String,
}

/// Normalized numeric limits for an external provider model catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalProviderNumericConfig {
    pub context_window: u64,
    pub auto_compact_token_limit: u64,
}

/// Numeric setting rejected by external provider configuration normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderNumericField {
    ContextWindow,
    AutoCompactTokenLimit,
}

/// Reason an external provider numeric setting failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderNumericIssue {
    Empty,
    Whitespace,
    InvalidUnsignedInteger,
    MustBeGreaterThanOne,
}

/// Boundary or field error returned by external provider normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProviderConfigError {
    Mojo(MojoError),
    InvalidValue {
        field: ExternalProviderNumericField,
        issue: ExternalProviderNumericIssue,
    },
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeOpenAiScalarPolicy {
    ProviderName = 0,
    LargeContextModel = 1,
    PreferMaxContextModel = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeDeepSeekWebSearchToken {
    Auto,
    Off,
    OpenAiChat,
    Anthropic,
}
