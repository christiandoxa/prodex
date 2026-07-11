//! RTK noisy shell command catalog.

pub(super) struct RtkNoisyShellCommand {
    pub(super) command: &'static str,
    pub(super) subcommands: &'static [&'static str],
}

pub(super) const RTK_NOISY_SHELL_COMMANDS: &[RtkNoisyShellCommand] = &[
    RtkNoisyShellCommand {
        command: "git",
        subcommands: &["diff", "show", "log", "status", "grep", "blame"],
    },
    RtkNoisyShellCommand {
        command: "cargo",
        subcommands: &["test", "build", "check", "clippy", "bench", "run"],
    },
    RtkNoisyShellCommand {
        command: "npm",
        subcommands: &["test", "run", "build", "install", "ci", "update", "audit"],
    },
    RtkNoisyShellCommand {
        command: "yarn",
        subcommands: &["test", "run", "build", "install", "add", "upgrade"],
    },
    RtkNoisyShellCommand {
        command: "pnpm",
        subcommands: &["test", "run", "build", "install", "add", "update"],
    },
    RtkNoisyShellCommand {
        command: "bun",
        subcommands: &["test", "run", "build", "install", "add"],
    },
    RtkNoisyShellCommand {
        command: "pytest",
        subcommands: &[],
    },
    RtkNoisyShellCommand {
        command: "go",
        subcommands: &["test", "build", "vet"],
    },
    RtkNoisyShellCommand {
        command: "docker",
        subcommands: &["build", "compose", "logs", "pull", "push", "run"],
    },
    RtkNoisyShellCommand {
        command: "kubectl",
        subcommands: &["logs", "describe", "get", "events", "top"],
    },
    RtkNoisyShellCommand {
        command: "rg",
        subcommands: &[],
    },
    RtkNoisyShellCommand {
        command: "find",
        subcommands: &[],
    },
    RtkNoisyShellCommand {
        command: "ls",
        subcommands: &[],
    },
    RtkNoisyShellCommand {
        command: "tree",
        subcommands: &[],
    },
    RtkNoisyShellCommand {
        command: "echo",
        subcommands: &[],
    },
    RtkNoisyShellCommand {
        command: "claw-compactor",
        subcommands: &["benchmark"],
    },
];
