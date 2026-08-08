use crate::caveman::caveman_tool_status;
use crate::discovery::{
    managed_optimizer_command_candidates, managed_optimizer_roots, path_dirs_from_env,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{BufReader, Read as _};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

const MAX_EXECUTABLE_DIGEST_BYTES: u64 = 512 * 1024 * 1024;
const TOOL_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const PONYTAIL_SOURCE: &str = "https://github.com/DietrichGebert/ponytail";
const TOOL_MANIFEST: &str = "prodex-tool.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolInstallManifest {
    schema_version: u32,
    id: String,
    version: String,
    source: String,
    commit: String,
    tree_sha256: String,
}

#[derive(Debug, Deserialize)]
struct CodexPluginManifest {
    name: String,
    version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptionalToolId {
    Caveman,
    Rtk,
    CodebaseMemoryMcp,
    PlaywrightMcp,
    Ponytail,
    Presidio,
}

impl OptionalToolId {
    pub const ALL: [Self; 6] = [
        Self::Caveman,
        Self::Rtk,
        Self::CodebaseMemoryMcp,
        Self::PlaywrightMcp,
        Self::Ponytail,
        Self::Presidio,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Caveman => "caveman",
            Self::Rtk => "rtk",
            Self::CodebaseMemoryMcp => "codebase-memory-mcp",
            Self::PlaywrightMcp => "playwright-mcp",
            Self::Ponytail => "ponytail",
            Self::Presidio => "presidio",
        }
    }
}

impl fmt::Display for OptionalToolId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for OptionalToolId {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "caveman" => Ok(Self::Caveman),
            "rtk" => Ok(Self::Rtk),
            "codebase-memory-mcp" | "codebase-memory" | "cbm" => Ok(Self::CodebaseMemoryMcp),
            "playwright" | "playwright-mcp" => Ok(Self::PlaywrightMcp),
            "ponytail" => Ok(Self::Ponytail),
            "presidio" => Ok(Self::Presidio),
            _ => Err(format!("unknown optional tool {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Command,
    CodexPlugin,
    McpServer,
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolCapability {
    Codex,
    Claude,
    ShellCompression,
    StructuralNavigation,
    BrowserAutomation,
    SimplicityReview,
    Redaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolDescriptor {
    pub id: OptionalToolId,
    pub kind: ToolKind,
    pub capabilities: &'static [ToolCapability],
}

pub const fn optional_tool_descriptor(id: OptionalToolId) -> ToolDescriptor {
    let (kind, capabilities): (ToolKind, &'static [ToolCapability]) = match id {
        OptionalToolId::Caveman => (
            ToolKind::CodexPlugin,
            &[ToolCapability::Codex, ToolCapability::Claude],
        ),
        OptionalToolId::Rtk => (ToolKind::Command, &[ToolCapability::ShellCompression]),
        OptionalToolId::CodebaseMemoryMcp => {
            (ToolKind::McpServer, &[ToolCapability::StructuralNavigation])
        }
        OptionalToolId::PlaywrightMcp => {
            (ToolKind::McpServer, &[ToolCapability::BrowserAutomation])
        }
        OptionalToolId::Ponytail => (ToolKind::CodexPlugin, &[ToolCapability::SimplicityReview]),
        OptionalToolId::Presidio => (ToolKind::Service, &[ToolCapability::Redaction]),
    };
    ToolDescriptor {
        id,
        kind,
        capabilities,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolDiscoverySource {
    ManagedRoot,
    Path,
    LocalService,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResolvedTool {
    pub descriptor: ToolDescriptor,
    pub source: ToolDiscoverySource,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolHealthStatus {
    Installed,
    Missing,
    Invalid,
    Degraded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolHealth {
    pub id: OptionalToolId,
    pub status: ToolHealthStatus,
    pub source: Option<ToolDiscoverySource>,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub digest: Option<String>,
    pub can_activate: bool,
    pub detail: String,
}

impl ToolHealth {
    pub(crate) fn installed(tool: ResolvedTool) -> Self {
        Self {
            id: tool.descriptor.id,
            status: ToolHealthStatus::Installed,
            source: Some(tool.source),
            path: tool.path,
            version: tool.version,
            digest: tool.digest,
            can_activate: true,
            detail: "installed and validated".to_string(),
        }
    }

    pub(crate) fn missing(id: OptionalToolId, detail: impl Into<String>) -> Self {
        Self {
            id,
            status: ToolHealthStatus::Missing,
            source: None,
            path: None,
            version: None,
            digest: None,
            can_activate: false,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionalToolSet(BTreeSet<OptionalToolId>);

impl OptionalToolSet {
    pub fn super_defaults() -> Self {
        Self(BTreeSet::from([
            OptionalToolId::Caveman,
            OptionalToolId::Rtk,
            OptionalToolId::CodebaseMemoryMcp,
            OptionalToolId::Ponytail,
        ]))
    }

    pub fn insert(&mut self, id: OptionalToolId) -> bool {
        self.0.insert(id)
    }

    pub fn remove(&mut self, id: OptionalToolId) -> bool {
        self.0.remove(&id)
    }

    pub fn contains(&self, id: OptionalToolId) -> bool {
        self.0.contains(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = OptionalToolId> + '_ {
        self.0.iter().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<OptionalToolId> for OptionalToolSet {
    fn from_iter<T: IntoIterator<Item = OptionalToolId>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolActivation {
    pub tool: ResolvedTool,
    pub required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolActivationPlan {
    pub activations: Vec<ToolActivation>,
    pub unavailable: Vec<ToolHealth>,
}

pub fn resolve_optional_tools(
    selected: &OptionalToolSet,
    required: &OptionalToolSet,
) -> ToolActivationPlan {
    let mut plan = ToolActivationPlan::default();
    for id in selected.iter() {
        if id == OptionalToolId::Caveman {
            match crate::resolve_caveman() {
                Ok(tool) => plan.activations.push(ToolActivation {
                    tool,
                    required: required.contains(id),
                }),
                Err(_) => plan.unavailable.push(caveman_tool_status()),
            }
            continue;
        }
        let health = optional_tool_status(id);
        if health.status == ToolHealthStatus::Installed {
            plan.activations.push(ToolActivation {
                tool: ResolvedTool {
                    descriptor: optional_tool_descriptor(id),
                    source: health.source.unwrap_or(ToolDiscoverySource::ManagedRoot),
                    path: health.path.clone(),
                    version: health.version.clone(),
                    digest: health.digest.clone(),
                },
                required: required.contains(id),
            });
        } else {
            plan.unavailable.push(health);
        }
    }
    plan
}

pub fn optional_tool_status(id: OptionalToolId) -> ToolHealth {
    match id {
        OptionalToolId::Caveman => caveman_tool_status(),
        OptionalToolId::Rtk => command_tool_status(id, "rtk"),
        OptionalToolId::CodebaseMemoryMcp => command_tool_status(id, "codebase-memory-mcp"),
        OptionalToolId::PlaywrightMcp => playwright_tool_status(),
        OptionalToolId::Ponytail => ponytail_tool_status(),
        OptionalToolId::Presidio => ToolHealth {
            id,
            status: ToolHealthStatus::Degraded,
            source: Some(ToolDiscoverySource::LocalService),
            path: None,
            version: None,
            digest: None,
            can_activate: false,
            detail: "service health is resolved by prodex doctor".to_string(),
        },
    }
}

fn command_tool_status(id: OptionalToolId, command: &str) -> ToolHealth {
    let roots = managed_optimizer_roots();
    for root in &roots {
        for candidate in managed_optimizer_command_candidates(root, command) {
            if candidate.is_file() {
                return resolved_managed_command(id, root, candidate, command_probe_args(id))
                    .map(ToolHealth::installed)
                    .unwrap_or_else(|error| invalid_tool(id, error));
            }
        }
    }
    for root in path_dirs_from_env() {
        let candidate = root.join(command);
        if candidate.is_file() {
            return resolved_command_tool(
                id,
                candidate,
                ToolDiscoverySource::Path,
                command_probe_args(id),
            )
            .map(ToolHealth::installed)
            .unwrap_or_else(|error| invalid_tool(id, error));
        }
        #[cfg(windows)]
        for suffix in ["exe", "cmd", "bat"] {
            let candidate = root.join(format!("{command}.{suffix}"));
            if candidate.is_file() {
                return resolved_command_tool(
                    id,
                    candidate,
                    ToolDiscoverySource::Path,
                    command_probe_args(id),
                )
                .map(ToolHealth::installed)
                .unwrap_or_else(|error| invalid_tool(id, error));
            }
        }
    }
    ToolHealth::missing(
        id,
        format!("{command} was not found in managed roots or PATH"),
    )
}

fn command_probe_args(id: OptionalToolId) -> &'static [&'static str] {
    match id {
        OptionalToolId::CodebaseMemoryMcp => &["--help"],
        _ => &["--version"],
    }
}

fn playwright_tool_status() -> ToolHealth {
    let id = OptionalToolId::PlaywrightMcp;
    let Some(node) = find_path_command("node") else {
        return ToolHealth::missing(id, "Node.js 18+ was not found on PATH");
    };
    let node_output = match crate::process::probe_command(&node, &["--version"], TOOL_PROBE_TIMEOUT)
    {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            return invalid_tool(
                id,
                anyhow::anyhow!("Node.js version check exited with {}", output.status),
            );
        }
        Err(error) => return invalid_tool(id, error),
    };
    let node_version = match probe_first_line(&node_output) {
        Ok(version) => version,
        Err(error) => return invalid_tool(id, error),
    };
    let major = node_version
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|value| value.parse::<u64>().ok());
    if !major.is_some_and(|major| major >= 18) {
        return ToolHealth::missing(id, format!("Node.js 18+ is required; found {node_version}"));
    }
    let Some(npx) = find_path_command("npx") else {
        return ToolHealth::missing(id, "npx was not found on PATH");
    };
    resolved_command_tool(id, npx, ToolDiscoverySource::Path, &["--version"])
        .map(ToolHealth::installed)
        .unwrap_or_else(|error| invalid_tool(id, error))
}

fn find_path_command(command: &str) -> Option<PathBuf> {
    for root in path_dirs_from_env() {
        let candidate = root.join(command);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        for suffix in ["exe", "cmd", "bat"] {
            let candidate = root.join(format!("{command}.{suffix}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn resolved_managed_command(
    id: OptionalToolId,
    root: &Path,
    path: PathBuf,
    args: &[&str],
) -> Result<ResolvedTool> {
    let root_metadata = fs::symlink_metadata(root)
        .with_context(|| format!("failed to inspect {}", root.display()))?;
    anyhow::ensure!(
        root_metadata.is_dir() && !root_metadata.file_type().is_symlink(),
        "optional-tool root {} must be a real directory",
        root.display()
    );
    let root = root.canonicalize()?;
    let tool = resolved_command_tool(id, path, ToolDiscoverySource::ManagedRoot, args)?;
    anyhow::ensure!(
        tool.path
            .as_deref()
            .is_some_and(|path| path.starts_with(&root)),
        "managed optional-tool command escapes {}",
        root.display()
    );
    Ok(tool)
}

fn resolved_command_tool(
    id: OptionalToolId,
    path: PathBuf,
    source: ToolDiscoverySource,
    args: &[&str],
) -> Result<ResolvedTool> {
    let mut tool = resolved_file_tool(id, path, source)?;
    let program = tool
        .path
        .as_deref()
        .context("resolved command has no path")?;
    let output = crate::process::probe_command(program, args, TOOL_PROBE_TIMEOUT)?;
    anyhow::ensure!(
        output.status.success(),
        "{} health check exited with {}: {}",
        id,
        output.status,
        probe_first_line(&output).unwrap_or_else(|_| "invalid UTF-8 diagnostic".to_string())
    );
    tool.version = Some(probe_first_line(&output)?);
    Ok(tool)
}

fn probe_first_line(output: &crate::process::ProbeOutput) -> Result<String> {
    let stdout = std::str::from_utf8(&output.stdout).context("probe stdout was not UTF-8")?;
    let stderr = std::str::from_utf8(&output.stderr).context("probe stderr was not UTF-8")?;
    let line = stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("available");
    let suffix = if output.truncated {
        " (output truncated)"
    } else {
        ""
    };
    Ok(format!("{line}{suffix}"))
}

fn ponytail_tool_status() -> ToolHealth {
    ponytail_tool_status_with_node(find_path_command("node"))
}

fn ponytail_tool_status_with_node(node: Option<PathBuf>) -> ToolHealth {
    let id = OptionalToolId::Ponytail;
    let Some(node) = node else {
        return ToolHealth::missing(id, "Node.js was not found on PATH");
    };
    match crate::process::probe_command(&node, &["--version"], TOOL_PROBE_TIMEOUT) {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            return invalid_tool(
                id,
                anyhow::anyhow!("Node.js health check exited with {}", output.status),
            );
        }
        Err(error) => return invalid_tool(id, error),
    }
    for root in managed_optimizer_roots() {
        let versioned = root.join("ponytail").join(crate::PONYTAIL_VETTED_VERSION);
        let versioned_exists = match crate::tree::path_exists(&versioned) {
            Ok(exists) => exists,
            Err(error) => return invalid_tool(id, error),
        };
        if versioned_exists {
            return validate_ponytail_install(&root, &versioned)
                .map(ToolHealth::installed)
                .unwrap_or_else(|error| invalid_tool(id, error));
        }
    }
    ToolHealth::missing(
        id,
        format!(
            "expected ponytail/{}/{} under a managed optional-tool root",
            crate::PONYTAIL_VETTED_VERSION,
            TOOL_MANIFEST
        ),
    )
}

fn validate_ponytail_install(allowed_root: &Path, candidate: &Path) -> Result<ResolvedTool> {
    let candidate = validated_managed_directory(allowed_root, candidate)?;
    let manifest_path = candidate.join(TOOL_MANIFEST);
    let manifest: ToolInstallManifest =
        serde_json::from_slice(&crate::tree::read_bounded_file(&manifest_path, 64 * 1024)?)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    anyhow::ensure!(
        manifest.schema_version == 1,
        "unsupported Ponytail manifest schema"
    );
    anyhow::ensure!(
        manifest.id == "ponytail",
        "Ponytail manifest id must be ponytail"
    );
    anyhow::ensure!(
        manifest.version == crate::PONYTAIL_VETTED_VERSION,
        "Ponytail version is not vetted"
    );
    anyhow::ensure!(
        manifest.source == PONYTAIL_SOURCE,
        "unexpected Ponytail source"
    );
    anyhow::ensure!(
        manifest.commit == crate::PONYTAIL_VETTED_COMMIT,
        "Ponytail commit does not match vetted metadata"
    );
    anyhow::ensure!(
        manifest.tree_sha256 == crate::PONYTAIL_VETTED_TREE_SHA256,
        "Ponytail manifest tree digest does not match vetted metadata"
    );

    let plugin_path = candidate.join(".codex-plugin/plugin.json");
    let plugin: CodexPluginManifest =
        serde_json::from_slice(&crate::tree::read_bounded_file(&plugin_path, 256 * 1024)?)
            .with_context(|| format!("failed to parse {}", plugin_path.display()))?;
    anyhow::ensure!(
        plugin.name == "ponytail",
        "Codex plugin name must be ponytail"
    );
    anyhow::ensure!(
        plugin.version == crate::PONYTAIL_VETTED_VERSION,
        "Codex plugin version does not match vetted metadata"
    );
    anyhow::ensure!(
        candidate.join("hooks/claude-codex-hooks.json").is_file()
            && candidate.join("skills").is_dir(),
        "Ponytail installation is incomplete"
    );
    let digest = crate::tree::tree_sha256(&candidate, b"prodex-ponytail-tree-v1\0")?;
    anyhow::ensure!(
        digest == crate::PONYTAIL_VETTED_TREE_SHA256,
        "Ponytail tree digest mismatch"
    );
    Ok(ResolvedTool {
        descriptor: optional_tool_descriptor(OptionalToolId::Ponytail),
        source: ToolDiscoverySource::ManagedRoot,
        path: Some(candidate),
        version: Some(plugin.version),
        digest: Some(format!("sha256:{digest}")),
    })
}

fn validated_managed_directory(allowed_root: &Path, candidate: &Path) -> Result<PathBuf> {
    let root_metadata = fs::symlink_metadata(allowed_root)
        .with_context(|| format!("failed to inspect {}", allowed_root.display()))?;
    anyhow::ensure!(
        root_metadata.is_dir() && !root_metadata.file_type().is_symlink(),
        "optional-tool root {} must be a real directory",
        allowed_root.display()
    );
    let candidate_metadata = fs::symlink_metadata(candidate)
        .with_context(|| format!("failed to inspect {}", candidate.display()))?;
    anyhow::ensure!(
        candidate_metadata.is_dir() && !candidate_metadata.file_type().is_symlink(),
        "optional-tool installation {} must be a real directory",
        candidate.display()
    );
    let allowed_root = allowed_root.canonicalize()?;
    let candidate = candidate.canonicalize()?;
    anyhow::ensure!(
        candidate.starts_with(&allowed_root) && candidate != allowed_root,
        "optional-tool installation escapes its managed root"
    );
    Ok(candidate)
}

fn resolved_file_tool(
    id: OptionalToolId,
    path: PathBuf,
    source: ToolDiscoverySource,
) -> Result<ResolvedTool> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() || metadata.len() > MAX_EXECUTABLE_DIGEST_BYTES {
        anyhow::bail!("{} is not a bounded regular file", path.display());
    }
    Ok(ResolvedTool {
        descriptor: optional_tool_descriptor(id),
        source,
        digest: Some(sha256_file(&path, MAX_EXECUTABLE_DIGEST_BYTES)?),
        path: Some(path),
        version: None,
    })
}

fn invalid_tool(id: OptionalToolId, error: anyhow::Error) -> ToolHealth {
    ToolHealth {
        id,
        status: ToolHealthStatus::Invalid,
        source: None,
        path: None,
        version: None,
        digest: None,
        can_activate: false,
        detail: error.to_string(),
    }
}

fn sha256_file(path: &Path, limit: u64) -> Result<String> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut reader = BufReader::new(file).take(limit.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        anyhow::ensure!(total <= limit, "{} is too large", path.display());
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(&hasher.finalize()))
}

pub(crate) fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ponytail_is_missing_when_node_is_not_on_path() {
        let health = ponytail_tool_status_with_node(None);

        assert_eq!(health.status, ToolHealthStatus::Missing);
        assert!(health.detail.contains("Node.js was not found"));
    }
}
