//! Gemini native request project stamping.

pub fn gemini_provider_core_native_request_body_with_project(
    body: &[u8],
    project_id: Option<&str>,
) -> Result<Vec<u8>, serde_json::Error> {
    let Some(project_id) = project_id else {
        return Ok(body.to_vec());
    };
    let mut value = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(value) => value,
        Err(_) => return Ok(body.to_vec()),
    };
    gemini_provider_core_stamp_native_project(&mut value, project_id);
    serde_json::to_vec(&value)
}

fn gemini_provider_core_stamp_native_project(value: &mut serde_json::Value, project_id: &str) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let project = serde_json::Value::String(project_id.to_string());
    if object.contains_key("project") {
        object.insert("project".to_string(), project.clone());
    }
    if object.contains_key("projectId") {
        object.insert("projectId".to_string(), project.clone());
    }
    if object.contains_key("cloudaicompanionProject") {
        object.insert("cloudaicompanionProject".to_string(), project.clone());
    }
    gemini_provider_core_stamp_native_metadata_project(object.get_mut("metadata"), project_id);
    if let Some(request) = object.get_mut("request") {
        gemini_provider_core_stamp_native_project(request, project_id);
    }
}

fn gemini_provider_core_stamp_native_metadata_project(
    value: Option<&mut serde_json::Value>,
    project_id: &str,
) {
    let Some(metadata) = value.and_then(serde_json::Value::as_object_mut) else {
        return;
    };
    if metadata.contains_key("duetProject") {
        metadata.insert(
            "duetProject".to_string(),
            serde_json::Value::String(project_id.to_string()),
        );
    }
}
