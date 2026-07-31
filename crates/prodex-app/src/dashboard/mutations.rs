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
    AppState, AppStateIoExt, ProfileEntry, ProfileProvider, audit_log_event,
    create_codex_home_if_missing, ensure_path_is_unique, managed_profile_home_path,
    prepare_managed_codex_home,
};

impl DashboardServer {
    pub(super) fn handle_set_active(&self, mut request: Request) -> Result<()> {
        let payload: ActiveProfileRequest = match read_json_body(&mut request) {
            Ok(payload) => payload,
            Err(err) => return respond_error(request, dashboard_json_body_error_status(&err), err),
        };
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
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
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
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
        for result in [
            create_codex_home_if_missing(&codex_home),
            prepare_managed_codex_home(&self.paths, &codex_home),
            ensure_path_is_unique(&state, &codex_home),
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
        let mut state = match AppState::load(&self.paths) {
            Ok(state) => state,
            Err(err) => return respond_error(request, StatusCode(500), err),
        };
        if state.profiles.remove(&name).is_none() {
            return respond_status(
                request,
                StatusCode(404),
                "application/json",
                br#"{"error":"profile_not_found"}"#.to_vec(),
            );
        }
        if state.active_profile.as_deref() == Some(name.as_str()) {
            state.active_profile = state.profiles.keys().next().cloned();
        }
        if let Err(err) = state.save_with_removed_profiles(&self.paths, std::slice::from_ref(&name))
        {
            return respond_error(request, StatusCode(500), err);
        }
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
