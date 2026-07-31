use anyhow::{Result, anyhow};
use serde_json::json;
use tiny_http::{Request, StatusCode};

use super::DashboardServer;
use super::payloads::{ActiveProfileRequest, AddProfileRequest};
use super::server::{
    dashboard_json_body_error_status, percent_decode, read_json_body, respond_error,
    respond_json_result, respond_status,
};
use crate::{
    AppStateIoExt, ProfileEntry, ProfileLifecycleHomeAction, ProfileLifecyclePlan, ProfileProvider,
    acquire_profile_lifecycle_lock, audit_log_event, create_codex_home_if_missing,
    ensure_path_is_unique, finalize_recovered_profile_removals, lifecycle_profile_state,
    load_profile_state_with_profile_recovery_locked, managed_profile_home_path,
    persist_pruned_profile_runtime_sidecars, prepare_managed_codex_home,
    prune_removed_profile_metadata, write_profile_lifecycle_plan,
};

impl DashboardServer {
    pub(super) fn handle_set_active(&self, mut request: Request) -> Result<()> {
        let payload: ActiveProfileRequest = match read_json_body(&mut request) {
            Ok(payload) => payload,
            Err(err) => return respond_error(request, dashboard_json_body_error_status(&err), err),
        };
        let _lock = match acquire_profile_lifecycle_lock(&self.paths) {
            Ok(lock) => lock,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        let (mut state, lifecycle_recovery) =
            match load_profile_state_with_profile_recovery_locked(&self.paths, true) {
                Ok(state) => state,
                Err(err) => return respond_error(request, StatusCode(500), err),
            };
        if let Err(err) = finalize_recovered_profile_removals(
            &self.paths,
            &state.profiles,
            &lifecycle_recovery.pending_removal_journals,
        ) {
            return respond_error(request, StatusCode(500), err);
        }
        if !state.profiles.contains_key(&payload.profile) {
            return respond_error(
                request,
                StatusCode(404),
                anyhow!("profile '{}' is missing", payload.profile),
            );
        }
        state.active_profile = Some(payload.profile);
        if let Err(err) = state.save(&self.paths) {
            return respond_error(request, StatusCode(500), err);
        }
        respond_json_result(
            request,
            audit_log_event(
                "dashboard",
                "profile_activate",
                "success",
                json!({ "profile_name": state.active_profile }),
            )
            .map(|()| json!({ "status": "ok", "activeProfile": state.active_profile })),
        )
    }

    pub(super) fn handle_add_profile(&self, mut request: Request) -> Result<()> {
        let payload: AddProfileRequest = match read_json_body(&mut request) {
            Ok(payload) => payload,
            Err(err) => return respond_error(request, dashboard_json_body_error_status(&err), err),
        };
        let name = payload.name.trim().to_string();
        if let Err(err) = prodex_profile_identity::validate_profile_name(&name) {
            return respond_error(request, StatusCode(400), err);
        }
        let _lock = match acquire_profile_lifecycle_lock(&self.paths) {
            Ok(lock) => lock,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        let (mut state, lifecycle_recovery) =
            match load_profile_state_with_profile_recovery_locked(&self.paths, true) {
                Ok(state) => state,
                Err(err) => return respond_error(request, StatusCode(500), err),
            };
        if let Err(err) = finalize_recovered_profile_removals(
            &self.paths,
            &state.profiles,
            &lifecycle_recovery.pending_removal_journals,
        ) {
            return respond_error(request, StatusCode(500), err);
        }
        if state.profiles.contains_key(&name) {
            return respond_error(
                request,
                StatusCode(409),
                anyhow!("profile '{}' already exists", name),
            );
        }
        let codex_home = match managed_profile_home_path(&self.paths, &name) {
            Ok(path) => path,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if let Err(err) = ensure_path_is_unique(&state, &codex_home) {
            return respond_error(request, StatusCode(500), err);
        }
        let lifecycle_path = match write_profile_lifecycle_plan(
            &self.paths,
            "manage",
            &ProfileLifecyclePlan {
                profile_states: match lifecycle_profile_state(
                    &name,
                    None,
                    Some(&ProfileEntry {
                        codex_home: codex_home.clone(),
                        managed: true,
                        email: None,
                        provider: ProfileProvider::Openai,
                    }),
                ) {
                    Ok(profile_state) => vec![profile_state],
                    Err(err) => return respond_error(request, StatusCode(500), err),
                },
                previous_active_profile: state.active_profile.clone(),
                next_active_profile: if payload.activate || state.active_profile.is_none() {
                    Some(name.clone())
                } else {
                    state.active_profile.clone()
                },
                home_actions: vec![ProfileLifecycleHomeAction::Create {
                    path: codex_home.display().to_string(),
                }],
                auth_journal_paths: Vec::new(),
            },
        ) {
            Ok(path) => path,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        for result in [
            create_codex_home_if_missing(&codex_home),
            prepare_managed_codex_home(&self.paths, &codex_home),
        ] {
            if let Err(err) = result {
                return respond_error(request, StatusCode(500), err);
            }
        }
        state.profiles.insert(
            name.clone(),
            ProfileEntry {
                codex_home: codex_home.clone(),
                managed: true,
                email: None,
                provider: ProfileProvider::Openai,
            },
        );
        if payload.activate || state.active_profile.is_none() {
            state.active_profile = Some(name.clone());
        }
        if let Err(err) = state.save(&self.paths) {
            return respond_error(request, StatusCode(500), err);
        }
        prodex_profile_export::cleanup_profile_lifecycle_journal(&lifecycle_path);
        respond_json_result(
            request,
            audit_log_event(
                "dashboard",
                "profile_add",
                "success",
                json!({
                    "profile_name": name,
                    "managed": true,
                    "activated": state.active_profile.as_deref() == Some(name.as_str()),
                }),
            )
            .map(|()| {
                json!({
                    "status": "ok",
                    "profile": name,
                    "activeProfile": state.active_profile,
                    "codexHome": codex_home.display().to_string(),
                })
            }),
        )
    }

    pub(super) fn handle_remove_profile(&self, request: Request, raw_name: String) -> Result<()> {
        let name = percent_decode(&raw_name);
        let _lock = match acquire_profile_lifecycle_lock(&self.paths) {
            Ok(lock) => lock,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        let (mut state, lifecycle_recovery) =
            match load_profile_state_with_profile_recovery_locked(&self.paths, true) {
                Ok(state) => state,
                Err(err) => return respond_error(request, StatusCode(500), err),
            };
        if let Err(err) = finalize_recovered_profile_removals(
            &self.paths,
            &state.profiles,
            &lifecycle_recovery.pending_removal_journals,
        ) {
            return respond_error(request, StatusCode(500), err);
        }
        let previous_state = state.clone();
        if state.profiles.remove(&name).is_none() {
            return respond_status(
                request,
                StatusCode(404),
                "application/json",
                br#"{"error":"profile_not_found"}"#.to_vec(),
            );
        }
        prune_removed_profile_metadata(&mut state, std::slice::from_ref(&name));
        let lifecycle_path = match write_profile_lifecycle_plan(
            &self.paths,
            "remove",
            &ProfileLifecyclePlan {
                profile_states: match lifecycle_profile_state(
                    &name,
                    previous_state.profiles.get(&name),
                    state.profiles.get(&name),
                ) {
                    Ok(profile_state) => vec![profile_state],
                    Err(err) => return respond_error(request, StatusCode(500), err),
                },
                previous_active_profile: previous_state.active_profile,
                next_active_profile: state.active_profile.clone(),
                home_actions: Vec::new(),
                auth_journal_paths: Vec::new(),
            },
        ) {
            Ok(path) => path,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if let Err(err) = state.save_with_removed_profiles(&self.paths, std::slice::from_ref(&name))
        {
            return respond_error(request, StatusCode(500), err);
        }
        if let Err(err) = persist_pruned_profile_runtime_sidecars(&self.paths, &state.profiles) {
            return respond_error(request, StatusCode(500), err);
        }
        prodex_profile_export::cleanup_profile_lifecycle_journal(&lifecycle_path);
        respond_json_result(
            request,
            audit_log_event(
                "dashboard",
                "profile_remove",
                "success",
                json!({
                    "profile_name": name,
                    "active_profile": state.active_profile,
                }),
            )
            .map(|()| json!({ "status": "ok", "activeProfile": state.active_profile })),
        )
    }
}
