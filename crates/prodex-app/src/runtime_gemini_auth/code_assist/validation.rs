use super::{GeminiCodeAssistValidation, GeminiLoadCodeAssistResponse};
use serde_json::Value;

pub(super) fn gemini_validation_from_load_response(
    response: &GeminiLoadCodeAssistResponse,
) -> Option<GeminiCodeAssistValidation> {
    response
        .ineligible_tiers
        .as_deref()?
        .iter()
        .find(|tier| {
            tier.reason_code.as_deref() == Some("VALIDATION_REQUIRED")
                && tier
                    .validation_url
                    .as_deref()
                    .is_some_and(|url| !url.is_empty())
        })
        .map(|tier| GeminiCodeAssistValidation {
            url: tier
                .validation_url
                .as_deref()
                .and_then(trusted_gemini_validation_url),
            description: tier.reason_message.clone(),
            learn_more_url: None,
        })
}

pub(in crate::runtime_gemini_auth) fn gemini_validation_from_body(
    body: &str,
) -> Option<GeminiCodeAssistValidation> {
    let value: Value = serde_json::from_str(body).ok()?;
    let error = value.get("error")?;
    let details = error.get("details")?.as_array()?;
    let error_info = details.iter().find(|detail| {
        detail.get("@type").and_then(Value::as_str)
            == Some("type.googleapis.com/google.rpc.ErrorInfo")
            && detail.get("reason").and_then(Value::as_str) == Some("VALIDATION_REQUIRED")
            && detail
                .get("domain")
                .and_then(Value::as_str)
                .is_some_and(gemini_code_assist_domain_matches)
    })?;
    let help_link = details
        .iter()
        .find(|detail| {
            detail.get("@type").and_then(Value::as_str)
                == Some("type.googleapis.com/google.rpc.Help")
        })
        .and_then(|detail| detail.get("links"))
        .and_then(Value::as_array)
        .and_then(|links| links.first());
    let url = help_link
        .and_then(|link| link.get("url"))
        .and_then(Value::as_str)
        .and_then(trusted_gemini_validation_url)
        .or_else(|| {
            error_info
                .get("metadata")
                .and_then(|metadata| {
                    metadata
                        .get("validation_url")
                        .or_else(|| metadata.get("validation_link"))
                })
                .and_then(Value::as_str)
                .and_then(trusted_gemini_validation_url)
        });
    let description = help_link
        .and_then(|link| link.get("description"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            error_info
                .get("metadata")
                .and_then(|metadata| metadata.get("validation_error_message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            error
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let learn_more_url = details
        .iter()
        .find(|detail| {
            detail.get("@type").and_then(Value::as_str)
                == Some("type.googleapis.com/google.rpc.Help")
        })
        .and_then(|detail| detail.get("links"))
        .and_then(Value::as_array)
        .and_then(|links| {
            links.iter().find_map(|link| {
                let description = link.get("description").and_then(Value::as_str)?;
                description
                    .eq_ignore_ascii_case("learn more")
                    .then(|| {
                        link.get("url")
                            .and_then(Value::as_str)
                            .and_then(trusted_gemini_validation_url)
                    })
                    .flatten()
            })
        });
    Some(GeminiCodeAssistValidation {
        url,
        description,
        learn_more_url,
    })
}

fn gemini_code_assist_domain_matches(domain: &str) -> bool {
    domain
        .trim()
        .eq_ignore_ascii_case("cloudcode-pa.googleapis.com")
}

fn trusted_gemini_validation_url(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value.trim()).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let google_host = host == "google.com"
        || host.ends_with(".google.com")
        || host == "googleapis.com"
        || host.ends_with(".googleapis.com");
    if url.scheme() != "https"
        || !google_host
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port != 443)
    {
        return None;
    }
    Some(url.to_string())
}

pub(in crate::runtime_gemini_auth) fn gemini_validation_error_message(
    validation: &GeminiCodeAssistValidation,
) -> String {
    let mut message = validation
        .description
        .clone()
        .unwrap_or_else(|| "Gemini account validation required".to_string());
    if let Some(url) = validation.url.as_deref() {
        message.push_str(&format!(" Open: {url}"));
    }
    if let Some(url) = validation.learn_more_url.as_deref() {
        message.push_str(&format!(" Learn more: {url}"));
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_urls_allow_only_google_https_origins() {
        assert_eq!(
            trusted_gemini_validation_url("https://accounts.google.com/verify").as_deref(),
            Some("https://accounts.google.com/verify")
        );
        for url in [
            "javascript:alert(1)",
            "file:///tmp/validation",
            "http://accounts.google.com/verify",
            "https://accounts.google.com.evil.example/verify",
            "https://user@accounts.google.com/verify",
            "https://accounts.google.com:8443/verify",
        ] {
            assert_eq!(trusted_gemini_validation_url(url), None, "url={url}");
        }
    }
}
