use super::*;

#[path = "runtime_launch/arg0_cleanup.rs"]
mod arg0_cleanup;
#[path = "runtime_launch/force_proxy.rs"]
mod force_proxy;
#[path = "runtime_launch/openai_model_context.rs"]
mod openai_model_context;
#[path = "runtime_launch/profile_selection.rs"]
mod profile_selection;
#[path = "runtime_launch/provider_rewrite.rs"]
mod provider_rewrite;
#[path = "runtime_launch/proxy_state.rs"]
mod proxy_state;
#[path = "runtime_launch/run_command_strategy.rs"]
mod run_command_strategy;
#[path = "runtime_launch/secure_fixture.rs"]
mod secure_fixture;
use secure_fixture::{session_meta_line, temp_dir, write_runtime_launch_auth};
#[path = "runtime_launch/session_maintenance.rs"]
mod session_maintenance;
#[path = "runtime_launch/super_runtime.rs"]
mod super_runtime;

fn write_state(root: &std::path::Path, state: AppState) {
    std::fs::create_dir_all(root).unwrap();
    let paths = AppPaths::discover().unwrap();
    state.save(&paths).unwrap();
}
