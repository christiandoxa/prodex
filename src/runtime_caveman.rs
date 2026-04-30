use super::*;
use prodex_caveman_assets::{
    PRODEX_CAVEMAN_MARKETPLACE_NAME, PRODEX_CAVEMAN_PLUGIN_ID, PRODEX_CAVEMAN_SOURCE_REPO,
    install_caveman_marketplace, install_caveman_plugin_cache,
};

#[allow(unused_imports)]
pub(crate) use prodex_caveman_assets::PRODEX_CAVEMAN_FULL_ASSETS_ENV;
const PRODEX_CAVEMAN_HOOK_COMMAND: &str = "echo 'CAVEMAN MODE ACTIVE. Use $caveman full: terse, no filler, exact technical substance. Code/commits/security normal. Stop: normal mode.'";

struct CavemanLaunchStrategy {
    args: CavemanArgs,
    codex_args: Vec<OsString>,
    include_code_review: bool,
    mem_mode: Option<RuntimeMemTranscriptMode>,
    model_provider_override: Option<String>,
}

impl CavemanLaunchStrategy {
    fn new(args: CavemanArgs) -> Self {
        let (mem_mode, codex_args) = runtime_mem_extract_mode_with_detail(&args.codex_args);
        let (codex_args, include_code_review) =
            prepare_codex_launch_args(&codex_args, args.full_access);
        let model_provider_override =
            codex_cli_config_override_value(&codex_args, "model_provider");
        Self {
            args,
            codex_args,
            include_code_review,
            mem_mode,
            model_provider_override,
        }
    }
}

impl RuntimeLaunchStrategy for CavemanLaunchStrategy {
    fn runtime_request(&self) -> RuntimeLaunchRequest<'_> {
        RuntimeLaunchRequest {
            profile: self.args.profile.as_deref(),
            allow_auto_rotate: self.args.auto_rotate && !self.args.no_auto_rotate,
            skip_quota_check: self.args.skip_quota_check,
            base_url: self.args.base_url.as_deref(),
            upstream_no_proxy: self.args.no_proxy,
            include_code_review: self.include_code_review,
            force_runtime_proxy: false,
            model_provider_override: self.model_provider_override.as_deref(),
        }
    }

    fn build_plan(
        &self,
        prepared: &PreparedRuntimeLaunch,
        runtime_proxy: Option<&RuntimeProxyEndpoint>,
    ) -> Result<RuntimeLaunchPlan> {
        let runtime_args = runtime_proxy_codex_passthrough_args(runtime_proxy, &self.codex_args);
        let caveman_home = prepare_caveman_launch_home(&prepared.paths, &prepared.codex_home)?;
        if let Some(mem_mode) = self.mem_mode {
            ensure_runtime_mem_prodex_observer(&prepared.paths)?;
            ensure_runtime_mem_codex_watch_for_home_with_mode(&caveman_home, mem_mode)?;
        }
        let mut child = codex_child_plan(caveman_home.clone(), runtime_args);
        if self.args.no_proxy && runtime_proxy.is_none() {
            remove_upstream_proxy_env(&mut child);
        }
        Ok(RuntimeLaunchPlan::new(child).with_cleanup_path(caveman_home))
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

    configure_caveman_session_start_hook(&mut table);

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

fn configure_caveman_session_start_hook(table: &mut toml::Table) {
    let hooks = ensure_child_table(table, "hooks");
    let session_start = hooks
        .entry("SessionStart".to_string())
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    if !session_start.is_array() {
        *session_start = toml::Value::Array(Vec::new());
    }
    let Some(session_start_groups) = session_start.as_array_mut() else {
        unreachable!("SessionStart should be an array after insertion");
    };

    let mut command_hook = toml::Table::new();
    command_hook.insert(
        "type".to_string(),
        toml::Value::String("command".to_string()),
    );
    command_hook.insert(
        "command".to_string(),
        toml::Value::String(PRODEX_CAVEMAN_HOOK_COMMAND.to_string()),
    );

    let mut group = toml::Table::new();
    group.insert(
        "hooks".to_string(),
        toml::Value::Array(vec![toml::Value::Table(command_hook)]),
    );
    session_start_groups.push(toml::Value::Table(group));
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
