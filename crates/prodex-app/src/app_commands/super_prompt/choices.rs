use super::prompt_choice;
use crate::provider_display_name;
use anyhow::Result;
use prodex_mojo_core::launch::SuperPromptLabelKind;

pub(in crate::app_commands) fn prompt_super_choice(
    title: &str,
    choices: &[String],
    selected: usize,
    escape_selects_last: bool,
) -> Result<usize> {
    prompt_choice(
        "Prodex Super",
        "Super sub-agent prompt TUI",
        title,
        choices,
        selected,
        escape_selects_last,
    )
}

pub(in crate::app_commands) fn super_prompt_provider_choice_label(
    provider: prodex_provider_core::ProviderId,
    session_affinity: bool,
) -> String {
    prodex_mojo_core::launch::super_prompt_format_label(
        SuperPromptLabelKind::Provider,
        0,
        Some(provider_display_name(provider)),
        session_affinity,
    )
    .expect("Mojo Super provider label formatter returned invalid output")
}

pub(in crate::app_commands) fn super_prompt_provider_choice_index(
    providers: &[prodex_provider_core::ProviderId],
    current: Option<prodex_provider_core::ProviderId>,
) -> usize {
    if let Some(provider) = current {
        let values = providers
            .iter()
            .map(|provider| *provider as i64)
            .collect::<Vec<_>>();
        prodex_mojo_core::launch::super_prompt_value_index(&values, provider as i64, 0)
            .expect("Mojo Super provider selection planner returned invalid output")
    } else {
        prodex_mojo_core::launch::super_choice_initial_index(0, providers.len())
            .expect("Mojo Super provider selection planner returned invalid output")
    }
}

pub(in crate::app_commands) fn super_prompt_model_choice_label(
    choice: &prodex_provider_core::ProviderModelChoice,
    label: &str,
) -> String {
    let formatted = match choice {
        prodex_provider_core::ProviderModelChoice::ProviderDefault => {
            prodex_mojo_core::launch::super_prompt_format_label(
                SuperPromptLabelKind::ProviderDefault,
                0,
                None,
                false,
            )
        }
        prodex_provider_core::ProviderModelChoice::Model(_) => {
            prodex_mojo_core::launch::super_prompt_format_label(
                SuperPromptLabelKind::Value,
                0,
                Some(label),
                false,
            )
        }
        prodex_provider_core::ProviderModelChoice::Custom => {
            prodex_mojo_core::launch::super_prompt_format_label(
                SuperPromptLabelKind::CustomModel,
                0,
                None,
                false,
            )
        }
    };
    formatted.expect("Mojo Super model label formatter returned invalid output")
}

pub(in crate::app_commands) fn super_prompt_model_choice_id_label(
    choice: &prodex_provider_core::ProviderModelChoice,
) -> String {
    let label = match choice {
        prodex_provider_core::ProviderModelChoice::Model(model) => model.as_str(),
        prodex_provider_core::ProviderModelChoice::ProviderDefault
        | prodex_provider_core::ProviderModelChoice::Custom => "",
    };
    super_prompt_model_choice_label(choice, label)
}

pub(in crate::app_commands) fn super_prompt_model_choice_index(
    choices: &[prodex_provider_core::ProviderModelChoice],
    current_model: Option<&str>,
) -> usize {
    let models = choices
        .iter()
        .map(|choice| match choice {
            prodex_provider_core::ProviderModelChoice::Model(model) => Some(model.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    prodex_mojo_core::launch::super_prompt_choice_index(&models, current_model, true, 0)
        .expect("Mojo Super model selection planner returned invalid output")
}

pub(in crate::app_commands) fn super_prompt_effort_label(effort: Option<&str>) -> String {
    let kind = if effort.is_some() {
        SuperPromptLabelKind::Value
    } else {
        SuperPromptLabelKind::ProviderDefault
    };
    prodex_mojo_core::launch::super_prompt_format_label(kind, 0, effort, false)
        .expect("Mojo Super reasoning-effort label formatter returned invalid output")
}

pub(in crate::app_commands) fn super_prompt_effort_choice_index(
    efforts: &[Option<&str>],
    current: Option<&str>,
    case_insensitive: bool,
) -> usize {
    prodex_mojo_core::launch::super_prompt_choice_index(efforts, current, case_insensitive, 0)
        .expect("Mojo Super reasoning-effort selection planner returned invalid output")
}
