use super::fs_utils::{
    collect_files, copy_dir_limited, read_optional_text_limited, read_text_limited,
    write_executable_script, write_file_atomic_no_symlink,
};
use super::utils::{
    first_nonempty_line, safe_slug, shell_quote, strip_front_matter, toml_string_literal,
    unique_slug, yaml_string_literal,
};
use super::{
    GEMINI_COMPAT_FILE_LIMIT, GEMINI_EXTENSION_SCAN_LIMIT, GENERATED_MARKER,
    GENERATED_PROMPT_MARKER, GENERATED_SKILL_MARKER_FILE, GeminiExtension, extension_skill_dirs,
};
use anyhow::{Context, Result, bail};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn write_gemini_skills(codex_home: &Path, extensions: &[GeminiExtension]) -> Result<()> {
    let skills_root = codex_home.join(".agents").join("skills");
    let mut seen = BTreeSet::new();
    let mut planned = Vec::new();
    for extension in extensions {
        for skill_dir in extension_skill_dirs(extension) {
            let source_name = skill_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("skill")
                .to_string();
            let source_skill = skill_dir.join("SKILL.md");
            let Some(source_text) = read_text_limited(&source_skill, GEMINI_COMPAT_FILE_LIMIT)
            else {
                bail!("Gemini extension skill missing {}", source_skill.display());
            };
            let target_name = unique_slug(
                &format!(
                    "gemini-{}-{}",
                    safe_slug(&extension.name),
                    safe_slug(&source_name)
                ),
                &mut seen,
            );
            planned.push((extension, skill_dir, source_name, source_text, target_name));
        }
    }
    for (extension, _, _, _, target_name) in &planned {
        let target_dir = skills_root.join(target_name);
        if fs::symlink_metadata(&target_dir).is_ok()
            && read_text_limited(&target_dir.join(GENERATED_SKILL_MARKER_FILE), 4096).as_deref()
                != Some(extension.name.as_str())
        {
            bail!(
                "refusing to replace user-owned Gemini skill target {}",
                target_dir.display()
            );
        }
    }
    fs::create_dir_all(&skills_root)
        .with_context(|| format!("failed to create {}", skills_root.display()))?;
    remove_generated_skill_dirs(&skills_root)?;

    for (extension, skill_dir, source_name, source_text, target_name) in planned {
        let target_dir = skills_root.join(&target_name);
        copy_skill_dir(
            extension,
            &skill_dir,
            &target_dir,
            &source_name,
            &source_text,
            &target_name,
        )?;
    }
    Ok(())
}

pub(super) fn write_gemini_agents(codex_home: &Path, extensions: &[GeminiExtension]) -> Result<()> {
    let agents_root = codex_home.join("agents");
    let mut seen = BTreeSet::new();
    let mut planned = Vec::new();
    for extension in extensions {
        let Ok(paths) = collect_files(
            &extension.directory.join("agents"),
            "md",
            GEMINI_EXTENSION_SCAN_LIMIT,
        ) else {
            continue;
        };
        for path in paths {
            let Some(body) = read_text_limited(&path, GEMINI_COMPAT_FILE_LIMIT) else {
                continue;
            };
            let source_name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("agent")
                .to_string();
            let agent_name = unique_slug(
                &format!(
                    "gemini-{}-{}",
                    safe_slug(&extension.name),
                    safe_slug(&source_name)
                ),
                &mut seen,
            );
            planned.push((extension, source_name, body, agent_name));
        }
    }
    for (_, _, _, agent_name) in &planned {
        let target = agents_root.join(format!("{agent_name}.toml"));
        if fs::symlink_metadata(&target).is_ok()
            && !read_text_limited(&target, 4096)
                .is_some_and(|text| text.starts_with(&format!("# {GENERATED_MARKER}")))
        {
            bail!(
                "refusing to replace user-owned Gemini agent target {}",
                target.display()
            );
        }
    }
    fs::create_dir_all(&agents_root)
        .with_context(|| format!("failed to create {}", agents_root.display()))?;
    remove_generated_agent_files(&agents_root)?;

    for (extension, source_name, body, agent_name) in planned {
        let description = first_nonempty_line(&body).unwrap_or_else(|| {
            format!("Gemini extension {} agent {}.", extension.name, source_name)
        });
        let rendered = format!(
            "# {GENERATED_MARKER}\nname = {name}\ndescription = {description}\ndeveloper_instructions = {instructions}\n",
            name = toml_string_literal(&agent_name),
            description = toml_string_literal(&description),
            instructions = toml_string_literal(&strip_front_matter(&body)),
        );
        write_file_atomic_no_symlink(&agents_root.join(format!("{agent_name}.toml")), rendered)
            .with_context(|| format!("failed to write generated agent {agent_name}"))?;
    }
    Ok(())
}

pub(super) fn write_gemini_admin_helpers(codex_home: &Path) -> Result<()> {
    let bin_dir = codex_home.join("bin");
    fs::create_dir_all(&bin_dir)
        .with_context(|| format!("failed to create {}", bin_dir.display()))?;
    let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("prodex"));
    let scripts = [
        (
            bin_dir.join("prodex-gemini-refresh"),
            format!(
            "#!/usr/bin/env sh\n# {GENERATED_MARKER}\nexec {} __gemini-compat-refresh --codex-home {} \"$@\"\n",
            shell_quote(&current_exe.display().to_string()),
            shell_quote(&codex_home.display().to_string())
            ),
        ),
        (
            bin_dir.join("prodex-gemini-checkpoint-create"),
            r#"#!/usr/bin/env sh
# prodex-gemini-cli-compat
set -eu
name="${1:-checkpoint}"
safe="$(printf '%s' "$name" | tr -c 'A-Za-z0-9._-' '-')"
root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "not inside a git worktree" >&2
  exit 2
}
git rev-parse --verify HEAD >/dev/null 2>&1 || {
  echo "checkpoint requires a repository with an initial commit" >&2
  exit 2
}
cd "$root"
checkpoint_tmp="$(mktemp -d "${TMPDIR:-/tmp}/prodex-gemini-checkpoint.XXXXXX")"
trap 'rm -rf "$checkpoint_tmp"' EXIT HUP INT TERM
index="$checkpoint_tmp/index"
GIT_INDEX_FILE="$index" git read-tree HEAD
GIT_INDEX_FILE="$index" git add -A -- . ':(exclude).gemini/checkpoints'
GIT_INDEX_FILE="$index" git diff --cached --binary --full-index HEAD > "$checkpoint_tmp/worktree.diff"
git status --porcelain=v1 -- . ':(exclude).gemini/checkpoints' > "$checkpoint_tmp/worktree.status"
stamp="$(date -u +%Y%m%dT%H%M%SZ)"
dir=".gemini/checkpoints"
base="$dir/${stamp}-${safe}-$$"
mkdir -p "$dir"
if [ -e "$base.json" ] || [ -e "$base.status" ] || [ -e "$base.diff" ]; then
  echo "checkpoint target already exists: $base" >&2
  exit 3
fi
{
  printf '{\n'
  printf '  "format": "prodex-gemini-worktree-checkpoint",\n'
  printf '  "created_at": "%s",\n' "$stamp"
  printf '  "head": "%s",\n' "$(git rev-parse HEAD 2>/dev/null || true)"
  printf '  "diff": "%s.diff",\n' "$(basename "$base")"
  printf '  "status": "%s.status"\n' "$(basename "$base")"
  printf '}\n'
} > "$checkpoint_tmp/checkpoint.json"
mv "$checkpoint_tmp/worktree.diff" "$base.diff"
mv "$checkpoint_tmp/worktree.status" "$base.status"
mv "$checkpoint_tmp/checkpoint.json" "$base.json"
printf '%s\n' "$base.diff"
"#
            .to_string(),
        ),
        (
            bin_dir.join("prodex-gemini-checkpoint-restore"),
            r#"#!/usr/bin/env sh
# prodex-gemini-cli-compat
set -eu
file="${1:-}"
if [ -z "$file" ]; then
  echo "usage: prodex-gemini-checkpoint-restore .gemini/checkpoints/<checkpoint>.diff" >&2
  exit 2
fi
case "$file" in
  /*) ;;
  *) file="$PWD/$file" ;;
esac
root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
  echo "not inside a git worktree" >&2
  exit 2
}
cd "$root"
if [ -n "$(git status --porcelain=v1 -- . ':(exclude).gemini/checkpoints')" ]; then
  echo "worktree is dirty; inspect status and confirm before restoring" >&2
  exit 3
fi
if [ ! -s "$file" ]; then
  echo "checkpoint has no worktree changes"
  exit 0
fi
git apply --check "$file"
git apply --index "$file"
git reset --quiet
"#
            .to_string(),
        ),
    ];
    for (path, _) in &scripts {
        if let Some(contents) = read_optional_text_limited(path, 4096)?
            && !generated_admin_helper(path, &contents)
        {
            bail!(
                "refusing to replace user-owned Gemini helper {}",
                path.display()
            );
        }
    }
    for (path, script) in scripts {
        write_executable_script(&path, &script)?;
    }
    Ok(())
}

fn generated_admin_helper(path: &Path, contents: &str) -> bool {
    let marker = format!("# {GENERATED_MARKER}");
    if contents.lines().nth(1) == Some(marker.as_str()) {
        return true;
    }
    match path.file_name().and_then(|name| name.to_str()) {
        Some("prodex-gemini-refresh") => contents
            .lines()
            .nth(1)
            .is_some_and(|line| line.contains(" __gemini-compat-refresh --codex-home ")),
        Some("prodex-gemini-checkpoint-create") => {
            contents.contains("prodex-gemini-worktree-checkpoint")
        }
        Some("prodex-gemini-checkpoint-restore") => {
            contents.contains("usage: prodex-gemini-checkpoint-restore ")
        }
        _ => false,
    }
}

fn copy_skill_dir(
    extension: &GeminiExtension,
    source_dir: &Path,
    target_dir: &Path,
    source_name: &str,
    source_skill: &str,
    skill_name: &str,
) -> Result<()> {
    if fs::symlink_metadata(target_dir).is_ok() {
        if read_text_limited(&target_dir.join(GENERATED_SKILL_MARKER_FILE), 4096).as_deref()
            != Some(extension.name.as_str())
        {
            bail!(
                "refusing to replace user-owned Gemini skill target {}",
                target_dir.display()
            );
        }
        fs::remove_dir_all(target_dir)
            .with_context(|| format!("failed to remove {}", target_dir.display()))?;
    }
    fs::create_dir_all(target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))?;
    write_file_atomic_no_symlink(
        &target_dir.join(GENERATED_SKILL_MARKER_FILE),
        &extension.name,
    )
    .with_context(|| format!("failed to mark {}", target_dir.display()))?;
    copy_dir_limited(source_dir, target_dir, GEMINI_EXTENSION_SCAN_LIMIT)?;
    let description = yaml_string_literal(&format!(
        "Gemini extension {} skill {}.",
        extension.name, source_name
    ));
    let rewritten = format!(
        "---\nname: {skill_name}\ndescription: {description}\n---\n\n{}\n\n{}",
        GENERATED_PROMPT_MARKER,
        strip_front_matter(source_skill)
    );
    write_file_atomic_no_symlink(&target_dir.join("SKILL.md"), rewritten)
        .with_context(|| format!("failed to write {}", target_dir.join("SKILL.md").display()))?;
    Ok(())
}

fn remove_generated_skill_dirs(skills_root: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(skills_root) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir()
            && !metadata.file_type().is_symlink()
            && read_text_limited(&path.join(GENERATED_SKILL_MARKER_FILE), 4096).is_some()
        {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}

fn remove_generated_agent_files(agents_root: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(agents_root) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
            continue;
        }
        let Some(text) = read_text_limited(&path, 4096) else {
            continue;
        };
        if text.starts_with(&format!("# {GENERATED_MARKER}")) {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }
    Ok(())
}
