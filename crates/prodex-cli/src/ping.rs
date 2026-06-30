use clap::{Args, Subcommand};

#[derive(Subcommand, Debug)]
pub enum PingCommands {
    #[command(about = "Send the prompt `ping` once through every ready OpenAI/Codex profile.")]
    Openai(PingOpenaiArgs),
}

#[derive(Args, Debug)]
pub struct PingOpenaiArgs {
    /// Override the ChatGPT backend base URL used for quota readiness checks.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,
    /// Bypass proxy environment variables for quota readiness checks.
    #[arg(long)]
    pub no_proxy: bool,
}
