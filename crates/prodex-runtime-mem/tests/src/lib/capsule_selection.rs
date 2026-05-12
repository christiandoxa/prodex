use super::*;

fn capsule(
    id: &str,
    token_cost: usize,
    required: bool,
    project_path: Option<&str>,
    updated_at_seconds: Option<i64>,
    relevance: f32,
) -> RuntimeMemCapsuleMetadata {
    RuntimeMemCapsuleMetadata {
        id: id.to_string(),
        token_cost,
        required,
        project_path: project_path.map(PathBuf::from),
        updated_at_seconds,
        relevance,
    }
}

fn recall_capsule(
    capsule: RuntimeMemCapsuleMetadata,
    paths: &[&str],
    symbols: &[&str],
) -> RuntimeMemRecallCapsuleMetadata {
    RuntimeMemRecallCapsuleMetadata {
        capsule,
        paths: paths.iter().map(PathBuf::from).collect(),
        symbols: symbols.iter().map(|symbol| symbol.to_string()).collect(),
    }
}

#[path = "capsule_selection/auto_budget.rs"]
mod auto_budget;

#[path = "capsule_selection/base_selection.rs"]
mod base_selection;

#[path = "capsule_selection/recall_diet.rs"]
mod recall_diet;
