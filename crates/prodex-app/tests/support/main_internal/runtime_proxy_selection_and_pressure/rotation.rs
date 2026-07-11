use super::*;

#[path = "rotation/auth_usage.rs"]
mod auth_usage;
#[path = "rotation/profile_removal.rs"]
mod profile_removal;
#[path = "rotation/paths_usage.rs"]
mod paths_usage;
#[path = "rotation/affinity_persistence.rs"]
mod affinity_persistence;
#[path = "rotation/affinity_persistence_requeues.rs"]
mod affinity_persistence_requeues;

use crate::profile_identity::{parse_email_from_id_token, profile_name_from_email};

#[path = "rotation/continuation_cleanup.rs"]
mod continuation_cleanup;
