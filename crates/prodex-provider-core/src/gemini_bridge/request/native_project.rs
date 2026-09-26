//! Gemini native request project stamping.

pub fn gemini_provider_core_native_request_body_with_project(
    body: &[u8],
    project_id: Option<&str>,
) -> Result<Vec<u8>, serde_json::Error> {
    let Some(project_id) = project_id else {
        return Ok(body.to_vec());
    };
    let value = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(value) => value,
        Err(_) => return Ok(body.to_vec()),
    };
    // Mojo consumes raw bytes; canonicalize with Serde to preserve the old wire ordering.
    let body = serde_json::to_vec(&value)?;
    #[cfg(feature = "mojo")]
    {
        Ok(super::request_contents::gemini_bridge_request_native_project(&body, project_id))
    }
    #[cfg(not(feature = "mojo"))]
    {
        let project_id = serde_json::to_vec(project_id)?;
        let mut input = prodex_mojo_core::provider_constraints::GeminiBridgeRequestKernelInput::new(
            prodex_mojo_core::provider_constraints::GeminiBridgeRequestOperation::NativeProject,
        );
        input.primary = Some(&body);
        input.secondary = Some(&project_id);
        let body = prodex_mojo_core::provider_constraints::gemini_bridge_request_kernel(input)
            .map_err(|error| {
                <serde_json::Error as serde::ser::Error>::custom(format!(
                    "Mojo Gemini native project kernel failed: {error:?}"
                ))
            })?;
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::gemini_provider_core_native_request_body_with_project;
    use serde_json::json;

    #[test]
    fn native_project_stamps_canonical_json_and_preserves_bad_input() {
        let body = br#"{"z":1,"project":"old","metadata":{"duetProject":"old","keep":"\u96ea"},"request":{"projectId":"old","cloudaicompanionProject":"old","items":[1,2,3]}}"#;
        let stamped =
            gemini_provider_core_native_request_body_with_project(body, Some("project-雪🙂"))
                .unwrap();
        assert_eq!(
            stamped,
            serde_json::to_vec(&json!({
                "metadata": {"duetProject":"project-雪🙂", "keep":"雪"},
                "project":"project-雪🙂",
                "request": {
                    "cloudaicompanionProject":"project-雪🙂",
                    "items":[1,2,3],
                    "projectId":"project-雪🙂"
                },
                "z":1
            }))
            .unwrap()
        );
        let malformed = b"{\"project\":";
        assert_eq!(
            gemini_provider_core_native_request_body_with_project(malformed, Some("p")).unwrap(),
            malformed
        );
        assert_eq!(
            gemini_provider_core_native_request_body_with_project(malformed, None).unwrap(),
            malformed
        );
    }
}
