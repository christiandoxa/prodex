use super::super::runtime::{OpenAiTunnelCredentials, resolve_openai_tunnel_id};
use super::support::{labeled_value_lines, text_lines};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Modifier;
use ratatui::text::Line;
use std::fmt;
use zeroize::Zeroizing;
const OPENAI_TUNNEL_ID_INPUT_MAX_BYTES: usize = "tunnel_".len() + 32;
const OPENAI_API_KEY_INPUT_MAX_BYTES: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::expose) enum OpenAiSetupField {
    TunnelId,
    ApiKey,
}
pub(in crate::expose) struct OpenAiSetupState {
    tunnel_id: String,
    api_key: Zeroizing<String>,
}
impl fmt::Debug for OpenAiSetupState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiSetupState")
            .field("tunnel_id", &self.tunnel_id)
            .field("api_key", &"<redacted>")
            .finish()
    }
}
pub(super) enum OpenAiSetupInput {
    Ignored,
    Edited,
    Next,
    Credentials(OpenAiTunnelCredentials),
}
impl OpenAiSetupState {
    pub(super) fn new(explicit_tunnel_id: Option<&str>) -> Self {
        let tunnel_id = explicit_tunnel_id
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned)
            .or_else(|| std::env::var("CONTROL_PLANE_TUNNEL_ID").ok())
            .unwrap_or_default();
        Self {
            tunnel_id: tunnel_id.trim().to_owned(),
            api_key: Zeroizing::new(std::env::var("CONTROL_PLANE_API_KEY").unwrap_or_default()),
        }
    }
    pub(super) fn masked_api_key(&self) -> String {
        "*".repeat(self.api_key.chars().count())
    }
    pub(super) fn next_field(&self) -> Option<OpenAiSetupField> {
        if self.tunnel_id.is_empty() {
            Some(OpenAiSetupField::TunnelId)
        } else {
            self.api_key.is_empty().then_some(OpenAiSetupField::ApiKey)
        }
    }
    pub(super) fn handle_key(
        &mut self,
        field: OpenAiSetupField,
        key: KeyEvent,
    ) -> Result<OpenAiSetupInput> {
        match key.code {
            KeyCode::Backspace | KeyCode::Delete => {
                self.value_mut(field).pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value_mut(field).clear();
            }
            KeyCode::Char(character)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                self.append(field, &character.to_string());
            }
            KeyCode::Enter => return self.submit(field),
            _ => return Ok(OpenAiSetupInput::Ignored),
        }
        Ok(OpenAiSetupInput::Edited)
    }
    fn submit(&mut self, field: OpenAiSetupField) -> Result<OpenAiSetupInput> {
        let tunnel_id = resolve_openai_tunnel_id(Some(&self.tunnel_id))?;
        self.tunnel_id.clone_from(&tunnel_id);
        Ok(match field {
            OpenAiSetupField::TunnelId => OpenAiSetupInput::Next,
            OpenAiSetupField::ApiKey => OpenAiSetupInput::Credentials(
                OpenAiTunnelCredentials::new(tunnel_id, self.api_key.to_string())?,
            ),
        })
    }
    fn value_mut(&mut self, field: OpenAiSetupField) -> &mut String {
        match field {
            OpenAiSetupField::TunnelId => &mut self.tunnel_id,
            OpenAiSetupField::ApiKey => &mut self.api_key,
        }
    }
    pub(super) fn append(&mut self, field: OpenAiSetupField, text: &str) {
        let value = self.value_mut(field);
        let max_bytes = match field {
            OpenAiSetupField::TunnelId => OPENAI_TUNNEL_ID_INPUT_MAX_BYTES,
            OpenAiSetupField::ApiKey => OPENAI_API_KEY_INPUT_MAX_BYTES,
        };
        let mut remaining = max_bytes.saturating_sub(value.len());
        for character in text.chars() {
            if character.len_utf8() > remaining {
                break;
            }
            value.push(character);
            remaining -= character.len_utf8();
        }
    }
}
pub(in crate::expose) fn setup_body(
    state: Option<&OpenAiSetupState>,
    field: OpenAiSetupField,
    status: Option<&str>,
    width: usize,
) -> Vec<Line<'static>> {
    let Some(state) = state else {
        return vec![Line::styled(
            "OpenAI tunnel setup is unavailable",
            super::tui_error_style(),
        )];
    };
    let masked_api_key = state.masked_api_key();
    let tunnel_label = match field {
        OpenAiSetupField::TunnelId => "Tunnel ID*",
        OpenAiSetupField::ApiKey => "Tunnel ID",
    };
    let tunnel_value = match state.tunnel_id.is_empty() {
        true => "<enter tunnel_<32 lowercase letters or digits>>",
        false => state.tunnel_id.as_str(),
    };
    let api_key_label = match field {
        OpenAiSetupField::TunnelId => "API key",
        OpenAiSetupField::ApiKey => "API key*",
    };
    let api_key_value = match masked_api_key.is_empty() {
        true => "<enter a control-plane key>",
        false => masked_api_key.as_str(),
    };
    let mut lines = vec![Line::styled(
        "OpenAI Secure MCP Tunnel setup",
        super::tui_primary_style().add_modifier(Modifier::BOLD),
    )];
    lines.extend(labeled_value_lines(
        tunnel_label,
        tunnel_value,
        width,
        super::tui_primary_style(),
        super::tui_primary_style(),
    ));
    lines.extend(labeled_value_lines(
        api_key_label,
        api_key_value,
        width,
        super::tui_primary_style(),
        super::tui_primary_style(),
    ));
    if let Some(status) = status {
        lines.extend(text_lines(status, width, super::tui_error_style()));
    }
    lines
}
