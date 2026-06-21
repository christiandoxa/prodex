use crate::AppPaths;
use anyhow::Result;
use prodex_cli::MemoryMcpArgs;
use std::path::{Path, PathBuf};

pub(crate) fn handle_memory_mcp(args: MemoryMcpArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let store = args
        .store
        .unwrap_or_else(|| default_memory_store_path(&paths));
    prodex_memory::run_memory_mcp_stdio(&store)
}

pub(crate) fn default_memory_store_path(paths: &AppPaths) -> PathBuf {
    prodex_memory::default_memory_store_path(&paths.root)
}

pub(crate) fn memory_store_ready(path: &Path) -> Result<()> {
    prodex_memory::memory_store_ready(path)
}
