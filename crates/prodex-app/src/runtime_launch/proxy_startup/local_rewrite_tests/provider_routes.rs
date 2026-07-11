use super::super::deepseek_rewrite::RuntimeDeepSeekWebSearchMode;
use super::super::local_rewrite::{
    RuntimeLocalRewriteProviderOptions, runtime_local_rewrite_remote_compact_unsupported_message,
};

#[test]
fn deepseek_remote_compact_reports_clear_unsupported_message() {
    let message = runtime_local_rewrite_remote_compact_unsupported_message(
        &RuntimeLocalRewriteProviderOptions::DeepSeek {
            api_keys: vec!["deepseek-key".to_string()],
            strict_tools: false,
            beta_base_url: "https://api.deepseek.com/beta".to_string(),
            web_search_mode: RuntimeDeepSeekWebSearchMode::Auto,
        },
    );

    assert_eq!(
        message,
        "DeepSeek provider does not support Codex remote compact yet"
    );
}
