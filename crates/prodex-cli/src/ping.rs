use clap::{Args, Subcommand};
use std::fmt;

#[derive(Subcommand, Debug)]
pub enum PingCommands {
    #[command(about = "Send a minimal application request through the OpenAI/Codex runtime path.")]
    Openai(PingOpenaiArgs),
}

#[derive(Args)]
pub struct PingOpenaiArgs {
    /// Probe only this OpenAI profile; omitted probes every configured eligible OpenAI profile.
    #[arg(short, long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Model passed through to the normal Codex request path.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,
    /// Override the ChatGPT backend base URL used by the normal OpenAI request path.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// Bypass proxy environment variables for the normal OpenAI request path.
    #[arg(long)]
    pub no_proxy: bool,
    /// Emit one stable JSON result instead of human-readable output.
    #[arg(long)]
    pub json: bool,
}

impl fmt::Debug for PingOpenaiArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PingOpenaiArgs")
            .field("profile_configured", &self.profile.is_some())
            .field("model_configured", &self.model.is_some())
            .field("base_url_configured", &self.base_url.is_some())
            .field("no_proxy", &self.no_proxy)
            .field("json", &self.json)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_debug_redacts_base_url_through_command_wrapper() {
        let sentinel = "ping-debug-secret-sentinel";
        let command = PingCommands::Openai(PingOpenaiArgs {
            profile: None,
            model: None,
            base_url: Some(format!("https://user:{sentinel}@example.test")),
            no_proxy: false,
            json: false,
        });

        let rendered = format!("{command:?}");
        assert!(rendered.contains("base_url_configured: true"), "{rendered}");
        assert!(!rendered.contains(sentinel), "{rendered}");
    }
}
