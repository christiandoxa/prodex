use super::*;

const PRODEX_CAVEMAN_MARKETPLACE_NAME: &str = "prodex-caveman";
const PRODEX_CAVEMAN_PLUGIN_NAME: &str = "caveman";
const PRODEX_CAVEMAN_PLUGIN_VERSION: &str = "0.1.0";
const PRODEX_CAVEMAN_PLUGIN_ID: &str = "caveman@prodex-caveman";
const PRODEX_CAVEMAN_SOURCE_REPO: &str = "https://github.com/JuliusBrussee/caveman.git";
const PRODEX_CAVEMAN_HOOK_COMMAND: &str = "echo 'CAVEMAN MODE ACTIVE. Rules: Drop articles/filler/pleasantries/hedging. Fragments OK. Short synonyms. Pattern: [thing] [action] [reason]. [next step]. Not: Sure! I would be happy to help you with that. Yes: Bug in auth middleware. Fix: Code/commits/security: write normal. User says stop caveman or normal mode to deactivate.'";

struct EmbeddedCavemanFile {
    relative_path: &'static str,
    contents: &'static str,
}

const CAVEMAN_PLUGIN_FILES: &[EmbeddedCavemanFile] = &[
    EmbeddedCavemanFile {
        relative_path: ".codex-plugin/plugin.json",
        contents: include_str!("caveman_assets/.codex-plugin/plugin.json"),
    },
    EmbeddedCavemanFile {
        relative_path: "assets/caveman-small.svg",
        contents: include_str!("caveman_assets/assets/caveman-small.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "assets/caveman.svg",
        contents: include_str!("caveman_assets/assets/caveman.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/SKILL.md",
        contents: include_str!("caveman_assets/skills/caveman/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/agents/openai.yaml",
        contents: include_str!("caveman_assets/skills/caveman/agents/openai.yaml"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/assets/caveman-small.svg",
        contents: include_str!("caveman_assets/skills/caveman/assets/caveman-small.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/caveman/assets/caveman.svg",
        contents: include_str!("caveman_assets/skills/caveman/assets/caveman.svg"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/SKILL.md",
        contents: include_str!("caveman_assets/skills/compress/SKILL.md"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/__init__.py",
        contents: include_str!("caveman_assets/skills/compress/scripts/__init__.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/__main__.py",
        contents: include_str!("caveman_assets/skills/compress/scripts/__main__.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/benchmark.py",
        contents: include_str!("caveman_assets/skills/compress/scripts/benchmark.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/cli.py",
        contents: include_str!("caveman_assets/skills/compress/scripts/cli.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/compress.py",
        contents: include_str!("caveman_assets/skills/compress/scripts/compress.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/detect.py",
        contents: include_str!("caveman_assets/skills/compress/scripts/detect.py"),
    },
    EmbeddedCavemanFile {
        relative_path: "skills/compress/scripts/validate.py",
        contents: include_str!("caveman_assets/skills/compress/scripts/validate.py"),
    },
];

struct CavemanLaunchStrategy {
    args: CavemanArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    mem_mode: bool,
}

impl CavemanLaunchStrategy {
    fn new(args: CavemanArgs) -> Self {
        let (mem_mode, codex_args) = runtime_mem_extract_mode(&args.codex_args);
        let (codex_args, include_code_review) = prepare_codex_launch_args(&codex_args);
        Self {
            args,
            codex_args,
            include_code_review,
            mem_mode,
        }
    }
}

impl RuntimeLaunchStrategy for CavemanLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: !self.args.no_auto_rotate,
            skip_quota_check: self.args.skip_quota_check,
            base_url: self.args.base_url.as_deref(),
            include_code_review: self.include_code_review,
            force_runtime_proxy: false,
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &self.codex_args);
        let caveman_home = prepare_caveman_launch_home(&prepared.paths, &prepared.codex_home)?;
        if self.mem_mode {
            ensure_runtime_mem_prodex_observer(&prepared.paths)?;
            ensure_runtime_mem_codex_watch_for_home(&caveman_home)?;
        }
        Ok(RuntimeLaunchPlan::new(
            ChildProcessPlan::new(codex_bin(), caveman_home.clone()).with_args(runtime_args),
        )
        .with_cleanup_path(caveman_home))
    }
}

pub(super) fn handle_caveman(args: CavemanArgs) -> Result<()> {
    execute_runtime_launch(CavemanLaunchStrategy::new(args))
}

pub(super) fn prepare_caveman_launch_home(
    paths: &AppPaths,
    base_codex_home: &Path,
) -> Result<PathBuf> {
    let caveman_home = create_temporary_caveman_home(paths)?;
    if let Err(err) = copy_codex_home(base_codex_home, &caveman_home)
        .and_then(|_| configure_caveman_launch_home(&caveman_home))
    {
        let _ = fs::remove_dir_all(&caveman_home);
        return Err(err);
    }
    Ok(caveman_home)
}

pub(super) fn configure_caveman_launch_home(codex_home: &Path) -> Result<()> {
    localize_caveman_config_file(codex_home)?;
    configure_caveman_config(codex_home)?;
    install_caveman_marketplace(codex_home)?;
    install_caveman_plugin_cache(codex_home)?;
    write_caveman_hooks_file(codex_home)?;
    Ok(())
}

fn create_temporary_caveman_home(paths: &AppPaths) -> Result<PathBuf> {
    fs::create_dir_all(&paths.managed_profiles_root).with_context(|| {
        format!(
            "failed to create managed profile root {}",
            paths.managed_profiles_root.display()
        )
    })?;

    for attempt in 0..100 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let candidate = paths
            .managed_profiles_root
            .join(format!(".caveman-{}-{stamp}-{attempt}", std::process::id()));
        if candidate.exists() {
            continue;
        }
        create_codex_home_if_missing(&candidate)?;
        return Ok(candidate);
    }

    bail!("failed to allocate a temporary CODEX_HOME for Caveman")
}

fn localize_caveman_config_file(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    match fs::symlink_metadata(&config_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let contents = fs::read_to_string(&config_path)
                .with_context(|| format!("failed to read {}", config_path.display()))?;
            fs::remove_file(&config_path)
                .with_context(|| format!("failed to remove {}", config_path.display()))?;
            fs::write(&config_path, contents)
                .with_context(|| format!("failed to write {}", config_path.display()))?;
        }
        Ok(metadata) if metadata.is_dir() => {
            bail!("{} is a directory, expected a file", config_path.display());
        }
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            fs::write(&config_path, "")
                .with_context(|| format!("failed to write {}", config_path.display()))?;
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to inspect {}", config_path.display()));
        }
    }
    Ok(())
}

fn configure_caveman_config(codex_home: &Path) -> Result<()> {
    let config_path = codex_home.join("config.toml");
    let contents = fs::read_to_string(&config_path).unwrap_or_default();
    let mut table = if contents.trim().is_empty() {
        toml::Table::new()
    } else {
        match toml::from_str::<toml::Value>(&contents)
            .with_context(|| format!("failed to parse {}", config_path.display()))?
        {
            toml::Value::Table(table) => table,
            _ => bail!("{} did not parse as a TOML table", config_path.display()),
        }
    };

    let features = ensure_child_table(&mut table, "features");
    features.insert("plugins".to_string(), toml::Value::Boolean(true));
    features.insert("codex_hooks".to_string(), toml::Value::Boolean(true));
    table.insert(
        "suppress_unstable_features_warning".to_string(),
        toml::Value::Boolean(true),
    );

    let marketplaces = ensure_child_table(&mut table, "marketplaces");
    let caveman_marketplace = ensure_child_table(marketplaces, PRODEX_CAVEMAN_MARKETPLACE_NAME);
    caveman_marketplace.insert(
        "last_updated".to_string(),
        toml::Value::String(Local::now().to_rfc3339()),
    );
    caveman_marketplace.insert(
        "source_type".to_string(),
        toml::Value::String("git".to_string()),
    );
    caveman_marketplace.insert(
        "source".to_string(),
        toml::Value::String(PRODEX_CAVEMAN_SOURCE_REPO.to_string()),
    );
    caveman_marketplace.insert("ref".to_string(), toml::Value::String("main".to_string()));

    let plugins = ensure_child_table(&mut table, "plugins");
    let caveman_plugin = ensure_child_table(plugins, PRODEX_CAVEMAN_PLUGIN_ID);
    caveman_plugin.insert("enabled".to_string(), toml::Value::Boolean(true));

    let rendered = toml::to_string(&toml::Value::Table(table))
        .context("failed to render Caveman config overlay")?;
    fs::write(&config_path, rendered)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    Ok(())
}

fn ensure_child_table<'a>(parent: &'a mut toml::Table, key: &str) -> &'a mut toml::Table {
    if !matches!(parent.get(key), Some(toml::Value::Table(_))) {
        parent.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    match parent.get_mut(key) {
        Some(toml::Value::Table(table)) => table,
        _ => unreachable!("child table should exist after insertion"),
    }
}

fn install_caveman_marketplace(codex_home: &Path) -> Result<()> {
    let marketplace_root = caveman_marketplace_root(codex_home);
    let plugin_root = marketplace_root
        .join("plugins")
        .join(PRODEX_CAVEMAN_PLUGIN_NAME);
    fs::create_dir_all(marketplace_root.join(".agents/plugins")).with_context(|| {
        format!(
            "failed to create Caveman marketplace root {}",
            marketplace_root.display()
        )
    })?;
    write_caveman_plugin_tree(&plugin_root)?;
    let marketplace_manifest = serde_json::to_string_pretty(&serde_json::json!({
        "name": PRODEX_CAVEMAN_MARKETPLACE_NAME,
        "interface": {
            "displayName": "Prodex Caveman",
        },
        "plugins": [
            {
                "name": PRODEX_CAVEMAN_PLUGIN_NAME,
                "source": {
                    "source": "local",
                    "path": format!("./plugins/{PRODEX_CAVEMAN_PLUGIN_NAME}"),
                },
                "policy": {
                    "installation": "AVAILABLE",
                    "authentication": "ON_INSTALL",
                },
                "category": "Productivity",
            }
        ],
    }))
    .context("failed to serialize Caveman marketplace manifest")?;
    fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        marketplace_manifest,
    )
    .with_context(|| {
        format!(
            "failed to write {}",
            marketplace_root
                .join(".agents/plugins/marketplace.json")
                .display()
        )
    })?;
    Ok(())
}

fn install_caveman_plugin_cache(codex_home: &Path) -> Result<()> {
    let plugin_cache_base = codex_home
        .join("plugins/cache")
        .join(PRODEX_CAVEMAN_MARKETPLACE_NAME)
        .join(PRODEX_CAVEMAN_PLUGIN_NAME);
    if plugin_cache_base.exists() {
        fs::remove_dir_all(&plugin_cache_base)
            .with_context(|| format!("failed to clear {}", plugin_cache_base.display()))?;
    }
    write_caveman_plugin_tree(&plugin_cache_base.join(PRODEX_CAVEMAN_PLUGIN_VERSION))
}

fn write_caveman_plugin_tree(root: &Path) -> Result<()> {
    for file in CAVEMAN_PLUGIN_FILES {
        let path = root.join(file.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, file.contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

fn write_caveman_hooks_file(codex_home: &Path) -> Result<()> {
    let hooks_path = codex_home.join("hooks.json");
    let hooks = serde_json::json!({
        "hooks": {
            "SessionStart": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": PRODEX_CAVEMAN_HOOK_COMMAND,
                        }
                    ]
                }
            ]
        }
    });
    let rendered = serde_json::to_string_pretty(&hooks)
        .context("failed to serialize Caveman hooks configuration")?;
    fs::write(&hooks_path, rendered)
        .with_context(|| format!("failed to write {}", hooks_path.display()))?;
    Ok(())
}

fn caveman_marketplace_root(codex_home: &Path) -> PathBuf {
    codex_home
        .join(".tmp/marketplaces")
        .join(PRODEX_CAVEMAN_MARKETPLACE_NAME)
}
