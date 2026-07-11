use super::fs_utils::{collect_files, read_text_limited};
use super::utils::{
    GeminiCompatVars, safe_slug, translate_gemini_prompt_placeholders, unique_slug,
    yaml_string_literal,
};
use super::{
    GEMINI_COMPAT_FILE_LIMIT, GEMINI_EXTENSION_SCAN_LIMIT, GENERATED_PROMPT_MARKER,
    GeminiExtension, extension_skill_dirs, toml_string, toml_string_array,
};
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
struct GeneratedPrompt {
    slug: String,
    description: String,
    argument_hint: Option<String>,
    body: String,
}

pub(super) fn write_gemini_prompts(
    codex_home: &Path,
    extensions: &[GeminiExtension],
    cwd: Option<&Path>,
) -> Result<()> {
    let prompts_dir = codex_home.join("prompts");
    fs::create_dir_all(&prompts_dir)
        .with_context(|| format!("failed to create {}", prompts_dir.display()))?;
    remove_generated_prompt_files(&prompts_dir)?;

    let mut prompts = builtin_gemini_prompts();
    prompts.extend(collect_user_and_project_commands(cwd));
    for extension in extensions {
        prompts.extend(collect_extension_commands(extension, cwd));
        prompts.extend(collect_extension_agent_prompts(extension));
        prompts.extend(collect_extension_skill_prompts(extension));
    }

    let mut seen = existing_prompt_slugs(&prompts_dir);
    for mut prompt in prompts {
        prompt.slug = unique_slug(&prompt.slug, &mut seen);
        write_prompt_file(&prompts_dir, &prompt)?;
    }
    Ok(())
}

fn builtin_gemini_prompts() -> Vec<GeneratedPrompt> {
    vec![
        GeneratedPrompt {
            slug: "gemini-refresh".to_string(),
            description: "Refresh Gemini CLI compatibility projection in Codex home.".to_string(),
            argument_hint: Some("[--dry-run]".to_string()),
            body: concat!(
                "Refresh the Gemini CLI compatibility projection for this Codex profile.\n\n",
                "Run `prodex-gemini-refresh $ARGUMENTS` from the shell. This regenerates ",
                "Gemini settings/extension MCP servers, hooks, custom prompts, skills, custom agents, ",
                "and helper scripts in the active `CODEX_HOME`. Restart Codex or start a new chat ",
                "when you need newly generated prompt/skill/agent menus to be reloaded."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-extensions".to_string(),
            description: "Inspect Gemini extension compatibility state.".to_string(),
            argument_hint: Some("[FILTER=\"...\"]".to_string()),
            body: concat!(
                "Inspect Gemini extension compatibility for `$FILTER`.\n\n",
                "Check `~/.gemini/extensions`, project `.gemini/extensions`, ",
                "`extension-enablement.json`, generated Codex `config.toml` MCP entries named ",
                "`gemini_*`, generated `hooks.json` entries with `Gemini extension ...` status, ",
                "`prompts/*.md`, `.agents/skills`, and `agents/*.toml`. Do not edit unrelated files."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-mcp-reload".to_string(),
            description: "Refresh Gemini MCP settings and extension servers.".to_string(),
            argument_hint: Some("[SERVER=\"...\"]".to_string()),
            body: concat!(
                "Refresh Gemini MCP compatibility for `$SERVER`.\n\n",
                "Run `prodex-gemini-refresh` and then inspect generated `[mcp_servers.gemini_*]` ",
                "entries in `config.toml`. If Codex is already running, restart the session before ",
                "expecting new MCP tools to appear."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-memory-show".to_string(),
            description: "Show effective Gemini memory files for this workspace.".to_string(),
            argument_hint: Some("[SCOPE=all|project|global]".to_string()),
            body: concat!(
                "Show effective Gemini memory for scope `$SCOPE`.\n\n",
                "Read, summarize, and report the relevant files without modifying them: ",
                "`~/.gemini/GEMINI.md`, `~/.gemini/memory/INBOX.md`, ancestor `GEMINI.md` files, ",
                "`.gemini/memory/MEMORY.md`, `.gemini/memory/INBOX.md`, and active extension ",
                "`contextFileName` files."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-memory-refresh".to_string(),
            description: "Refresh and reconcile Gemini memory for this workspace.".to_string(),
            argument_hint: Some("[SCOPE=project|global|all]".to_string()),
            body: concat!(
                "Refresh Gemini memory for scope `$SCOPE`.\n\n",
                "Review the memory inbox, remove stale/transient notes, keep stable non-secret guidance, ",
                "and write concise updates to the appropriate Gemini memory file. Preserve unrelated content."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-memory-inbox".to_string(),
            description: "Review and reconcile Gemini memory inbox notes.".to_string(),
            argument_hint: Some("[SCOPE=project|global]".to_string()),
            body: concat!(
                "Review Gemini memory inbox notes for this workspace.\n\n",
                "- Project inbox: `.gemini/memory/INBOX.md`\n",
                "- Project private memory: `.gemini/memory/MEMORY.md`\n",
                "- Global inbox: `~/.gemini/memory/INBOX.md`\n",
                "- Global Gemini memory: `~/.gemini/GEMINI.md`\n\n",
                "Move stable, non-secret, recurring guidance into the appropriate Gemini memory file. ",
                "Keep transient notes out of durable memory. Scope hint: $SCOPE."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-remember".to_string(),
            description: "Add stable guidance to the Gemini memory inbox.".to_string(),
            argument_hint: Some("SCOPE=project|global NOTE=\"...\"".to_string()),
            body: concat!(
                "Add this stable memory note to Gemini memory without storing secrets.\n\n",
                "Scope: $SCOPE\n",
                "Note: $NOTE\n\n",
                "Use `.gemini/memory/INBOX.md` for project-scoped notes and ",
                "`~/.gemini/memory/INBOX.md` for global notes. Create parent directories if needed. ",
                "Keep entries concise and actionable."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-checkpoint-create".to_string(),
            description: "Create a Gemini-style workspace checkpoint.".to_string(),
            argument_hint: Some("[NAME=label]".to_string()),
            body: concat!(
                "Create a Gemini-style workspace checkpoint.\n\n",
                "Run `prodex-gemini-checkpoint-create $NAME`. It records git HEAD, status, ",
                "and a binary diff under `.gemini/checkpoints/`. Use this before risky mutating work."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-checkpoint-export".to_string(),
            description: "Prepare a Gemini-compatible request checkpoint export.".to_string(),
            argument_hint: Some("FILE=checkpoint.json".to_string()),
            body: concat!(
                "Prepare this session to export Gemini bridge checkpoints.\n\n",
                "Use `PRODEX_GEMINI_CHECKPOINT_EXPORT_FILE=$FILE` or request metadata ",
                "`gemini_checkpoint_export_file` on the next Gemini-backed request. ",
                "The bridge writes the translated Gemini `generateContent` request with model, ",
                "contents, tools, generation config, and system instruction for later import."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-checkpoint-restore".to_string(),
            description: "Restore a Gemini-style workspace checkpoint.".to_string(),
            argument_hint: Some("FILE=.gemini/checkpoints/name.diff".to_string()),
            body: concat!(
                "Restore a Gemini-style workspace checkpoint from `$FILE`.\n\n",
                "First inspect current `git status --short` and the checkpoint metadata. ",
                "If the user confirms restoration, run `prodex-gemini-checkpoint-restore $FILE`. ",
                "Do not discard unrelated user changes without explicit confirmation."
            )
            .to_string(),
        },
        GeneratedPrompt {
            slug: "gemini-rewind".to_string(),
            description: "Resume context from a Gemini request checkpoint file.".to_string(),
            argument_hint: Some("FILE=checkpoint.json".to_string()),
            body: concat!(
                "Plan a Gemini checkpoint rewind from `$FILE`.\n\n",
                "A future Gemini-backed run can import it with `PRODEX_GEMINI_CHECKPOINT_FILE=$FILE` ",
                "or request metadata `gemini_checkpoint_file`. Verify the file exists, summarize ",
                "what context it contains if readable, and avoid editing unrelated project files."
            )
            .to_string(),
        },
    ]
}

fn collect_user_and_project_commands(cwd: Option<&Path>) -> Vec<GeneratedPrompt> {
    let mut prompts = Vec::new();
    if let Some(home) = dirs::home_dir() {
        prompts.extend(collect_commands_from_dir(
            &home.join(".gemini").join("commands"),
            "",
            "user Gemini command",
            None,
        ));
    }
    if let Some(cwd) = cwd {
        prompts.extend(collect_commands_from_dir(
            &cwd.join(".gemini").join("commands"),
            "",
            "project Gemini command",
            None,
        ));
    }
    prompts
}

fn collect_extension_commands(
    extension: &GeminiExtension,
    cwd: Option<&Path>,
) -> Vec<GeneratedPrompt> {
    collect_commands_from_dir(
        &extension.directory.join("commands"),
        &safe_slug(&extension.name),
        "Gemini extension command",
        Some(&GeminiCompatVars::new(&extension.directory, cwd)),
    )
}

fn collect_commands_from_dir(
    directory: &Path,
    prefix: &str,
    source_label: &str,
    vars: Option<&GeminiCompatVars>,
) -> Vec<GeneratedPrompt> {
    let mut prompts = Vec::new();
    let Ok(paths) = collect_files(directory, "toml", GEMINI_EXTENSION_SCAN_LIMIT) else {
        return prompts;
    };
    for path in paths {
        let Some(text) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
            continue;
        };
        let Ok(value) = toml::from_str::<toml::Value>(&text) else {
            continue;
        };
        let relative = path.strip_prefix(directory).unwrap_or(path.as_path());
        let command_name = toml_string(&value, &["name"])
            .unwrap_or_else(|| relative.with_extension("").display().to_string());
        let slug = if prefix.is_empty() {
            safe_slug(&command_name)
        } else {
            format!("{}-{}", prefix, safe_slug(&command_name))
        };
        let description = toml_string(&value, &["description"])
            .unwrap_or_else(|| format!("{source_label}: {}", relative.display()));
        let argument_hint = toml_string(&value, &["argument-hint", "argumentHint"]);
        let raw_body = toml_string(&value, &["prompt", "body", "instruction", "instructions"])
            .or_else(|| {
                toml_string(&value, &["command"]).map(|command| {
                    format!(
                        "Run or adapt this Gemini command for the user's request:\n\n```sh\n{command}\n```"
                    )
                })
            })
            .unwrap_or_default();
        if raw_body.trim().is_empty() {
            continue;
        }
        let body = vars.map(|vars| vars.expand(&raw_body)).unwrap_or(raw_body);
        prompts.push(GeneratedPrompt {
            slug,
            description: description.clone(),
            argument_hint: argument_hint.clone(),
            body: translate_gemini_prompt_placeholders(&body),
        });
        for alias in toml_string_array(&value, &["aliases", "alias"]) {
            let alias_slug = if prefix.is_empty() {
                safe_slug(&alias)
            } else {
                format!("{}-{}", prefix, safe_slug(&alias))
            };
            prompts.push(GeneratedPrompt {
                slug: alias_slug,
                description: format!("{description} (alias)"),
                argument_hint: argument_hint.clone(),
                body: translate_gemini_prompt_placeholders(&body),
            });
        }
    }
    prompts
}

fn collect_extension_agent_prompts(extension: &GeminiExtension) -> Vec<GeneratedPrompt> {
    let mut prompts = Vec::new();
    let Ok(paths) = collect_files(
        &extension.directory.join("agents"),
        "md",
        GEMINI_EXTENSION_SCAN_LIMIT,
    ) else {
        return prompts;
    };
    for path in paths {
        let Some(body) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
            continue;
        };
        let slug = format!(
            "gemini-{}-agent-{}",
            safe_slug(&extension.name),
            safe_slug(
                path.file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("agent")
            )
        );
        prompts.push(GeneratedPrompt {
            slug,
            description: format!("Use Gemini extension {} agent guidance.", extension.name),
            argument_hint: Some("[TASK=\"...\"]".to_string()),
            body: format!(
                "Use this Gemini extension agent guidance for the task `$TASK`.\n\n{body}"
            ),
        });
    }
    prompts
}

fn collect_extension_skill_prompts(extension: &GeminiExtension) -> Vec<GeneratedPrompt> {
    let mut prompts = Vec::new();
    for skill_dir in extension_skill_dirs(extension) {
        let skill_name = skill_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("skill");
        let Some(body) = read_text_limited(&skill_dir.join("SKILL.md"), GEMINI_COMPAT_FILE_LIMIT)
        else {
            continue;
        };
        prompts.push(GeneratedPrompt {
            slug: format!(
                "gemini-{}-skill-{}",
                safe_slug(&extension.name),
                safe_slug(skill_name)
            ),
            description: format!(
                "Invoke Gemini extension {} skill {}.",
                extension.name, skill_name
            ),
            argument_hint: Some("[TASK=\"...\"]".to_string()),
            body: format!("Use this Gemini extension skill for `$TASK`.\n\n{body}"),
        });
    }
    prompts
}

fn write_prompt_file(prompts_dir: &Path, prompt: &GeneratedPrompt) -> Result<()> {
    let path = prompts_dir.join(format!("{}.md", prompt.slug));
    let mut contents = String::new();
    contents.push_str(GENERATED_PROMPT_MARKER);
    contents.push('\n');
    contents.push_str("---\n");
    contents.push_str(&format!(
        "description: {}\n",
        yaml_string_literal(&prompt.description)
    ));
    if let Some(argument_hint) = &prompt.argument_hint {
        contents.push_str(&format!(
            "argument-hint: {}\n",
            yaml_string_literal(argument_hint)
        ));
    }
    contents.push_str("---\n\n");
    contents.push_str(&prompt.body);
    contents.push('\n');
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn remove_generated_prompt_files(prompts_dir: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(prompts_dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let Some(text) = read_text_limited(&path, 4096) else {
            continue;
        };
        if text.contains(GENERATED_PROMPT_MARKER) {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn existing_prompt_slugs(prompts_dir: &Path) -> BTreeSet<String> {
    let mut slugs = BTreeSet::new();
    let Ok(entries) = fs::read_dir(prompts_dir) else {
        return slugs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            slugs.insert(safe_slug(stem));
        }
    }
    slugs
}
