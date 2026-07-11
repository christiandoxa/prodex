pub(super) struct EmbeddedCavemanFile {
    pub(super) relative_path: &'static str,
    pub(super) contents: &'static str,
}

pub(super) const CAVEMAN_CORE_PLUGIN_FILES: &[EmbeddedCavemanFile] = &[
    EmbeddedCavemanFile {
        relative_path: ".codex-plugin/plugin.json",
        contents: include_str!("../caveman_assets/.codex-plugin/plugin.json"),
    },
    EmbeddedCavemanFile {
        relative_path: "assets/caveman-small.svg",
        contents: include_str!("../caveman_assets/assets/caveman-small.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "assets/caveman.svg",
        contents: include_str!("../caveman_assets/assets/caveman.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "assets/caveman-dark.svg",
        contents: include_str!("../caveman_assets/assets/caveman-dark.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/SKILL.md",
        contents: include_str!("../caveman_assets/skills/caveman/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/agents/openai.yaml",
        contents: include_str!("../caveman_assets/skills/caveman/agents/openai.yaml"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/assets/caveman-small.svg",
        contents: include_str!("../caveman_assets/skills/caveman/assets/caveman-small.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/assets/caveman.svg",
        contents: include_str!("../caveman_assets/skills/caveman/assets/caveman.svg"),
    },
];

pub(super) const CAVEMAN_COMPRESS_PLUGIN_FILES: &[EmbeddedCavemanFile] = &[
    EmbeddedCavemanFile {
        relative_path: "skills/compress/SKILL.md",
        contents: include_str!("../caveman_assets/skills/compress/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/__init__.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/__init__.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/__main__.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/__main__.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/benchmark.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/benchmark.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/cli.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/cli.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/compress.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/compress.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/detect.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/detect.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/validate.py",
        contents: include_str!("../caveman_assets/skills/compress/scripts/validate.py"),
    },
];

pub(super) const CLAUDE_CAVEMAN_PLUGIN_FILES: &[EmbeddedCavemanFile] = &[
    EmbeddedCavemanFile {
        relative_path: ".claude-plugin/plugin.json",
        contents: include_str!("../caveman_assets/claude/.claude-plugin/plugin.json"),
    },
    EmbeddedCavemanFile {
        relative_path: "commands/caveman.toml",
        contents: include_str!("../caveman_assets/claude/commands/caveman.toml"),
    },
    EmbeddedCavemanFile {
        relative_path: "commands/caveman-commit.toml",
        contents: include_str!("../caveman_assets/claude/commands/caveman-commit.toml"),
    },
    EmbeddedCavemanFile {
        relative_path: "commands/caveman-review.toml",
        contents: include_str!("../caveman_assets/claude/commands/caveman-review.toml"),
    },
    EmbeddedCavemanFile {
        relative_path: "hooks/caveman-activate.js",
        contents: include_str!("../caveman_assets/claude/hooks/caveman-activate.js"),
    },
    EmbeddedCavemanFile {
        relative_path: "hooks/caveman-config.js",
        contents: include_str!("../caveman_assets/claude/hooks/caveman-config.js"),
    },
    EmbeddedCavemanFile {
        relative_path: "hooks/caveman-mode-tracker.js",
        contents: include_str!("../caveman_assets/claude/hooks/caveman-mode-tracker.js"),
    },
    EmbeddedCavemanFile {
        relative_path: "hooks/caveman-statusline.ps1",
        contents: include_str!("../caveman_assets/claude/hooks/caveman-statusline.ps1"),
    },
    EmbeddedCavemanFile {
        relative_path: "hooks/caveman-statusline.sh",
        contents: include_str!("../caveman_assets/claude/hooks/caveman-statusline.sh"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/SKILL.md",
        contents: include_str!("../caveman_assets/skills/caveman/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman-commit/SKILL.md",
        contents: include_str!("../caveman_assets/claude/skills/caveman-commit/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman-help/SKILL.md",
        contents: include_str!("../caveman_assets/claude/skills/caveman-help/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman-review/SKILL.md",
        contents: include_str!("../caveman_assets/claude/skills/caveman-review/SKILL.md"),
    },
];
