use clap::{Args, Subcommand};
use std::fmt;

#[derive(Subcommand, Debug)]
pub enum PingCommands {
    #[command(
        about = "Run a minimal authenticated end-to-end health check for each OpenAI/Codex profile."
    )]
    Openai(PingOpenaiArgs),
}

#[derive(Args)]
pub struct PingOpenaiArgs {
    /// Override the ChatGPT backend base URL used for quota readiness checks.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// Bypass proxy environment variables for quota readiness checks.
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
            base_url: Some(format!("https://user:{sentinel}@example.test")),
            no_proxy: false,
            json: false,
        });

        let rendered = format!("{command:?}");
        assert!(rendered.contains("base_url_configured: true"), "{rendered}");
        assert!(!rendered.contains(sentinel), "{rendered}");
    }
}
