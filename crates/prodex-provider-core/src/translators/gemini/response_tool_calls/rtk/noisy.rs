//! Gemini-specific RTK noisy shell command catalog.

pub(super) struct GeminiRtkNoisyShellCommand {
    pub(super) command: &'static str,
    pub(super) subcommands: &'static [&'static str],
}

pub(super) const GEMINI_RTK_NOISY_SHELL_COMMANDS: &[GeminiRtkNoisyShellCommand] = &[
    GeminiRtkNoisyShellCommand {
        command: "git",
        subcommands: &["diff", "show", "log", "status", "grep", "blame"],
    },
    GeminiRtkNoisyShellCommand {
        command: "cargo",
        subcommands: &["test", "build", "check", "clippy", "bench", "run"],
    },
    GeminiRtkNoisyShellCommand {
        command: "npm",
        subcommands: &["test", "run", "build", "install", "ci", "update", "audit"],
    },
    GeminiRtkNoisyShellCommand {
        command: "yarn",
        subcommands: &["test", "run", "build", "install", "add", "upgrade"],
    },
    GeminiRtkNoisyShellCommand {
        command: "pnpm",
        subcommands: &["test", "run", "build", "install", "add", "update"],
    },
    GeminiRtkNoisyShellCommand {
        command: "bun",
        subcommands: &["test", "run", "build", "install", "add"],
    },
    GeminiRtkNoisyShellCommand {
        command: "pytest",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "go",
        subcommands: &["test", "build", "vet"],
    },
    GeminiRtkNoisyShellCommand {
        command: "docker",
        subcommands: &["build", "compose", "logs", "pull", "push", "run"],
    },
    GeminiRtkNoisyShellCommand {
        command: "kubectl",
        subcommands: &["logs", "describe", "get", "events", "top"],
    },
    GeminiRtkNoisyShellCommand {
        command: "rg",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "find",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "ls",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "tree",
        subcommands: &[],
    },
    GeminiRtkNoisyShellCommand {
        command: "claw-compactor",
        subcommands: &["benchmark", "compact", "summarize"],
    },
    GeminiRtkNoisyShellCommand {
        command: "prodex-claw-compactor-auto",
        subcommands: &[],
    },
];
