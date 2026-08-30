use serde_json::{Value, json};

#[cfg(feature = "mojo")]
use prodex_mojo_core::rich::{GeminiResponseKernelInput, GeminiResponseKernelOperation};

#[cfg(feature = "mojo")]
use super::gemini_mojo_value;

pub fn gemini_provider_core_response_created_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::ResponseCreated);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.created",
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response": {"id": response_id},
    })
}

pub fn gemini_provider_core_response_completed_event(
    sequence_number: u64,
    created_at: u64,
    response: &Value,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let response = serde_json::to_string(response).expect("response serializes");
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::ResponseCompleted);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response = Some(&response);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.completed",
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response": response,
    })
}

pub fn gemini_provider_core_response_incomplete_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    reason: &str,
    message: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::ResponseIncomplete);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        input.reason = Some(reason);
        input.message = Some(message);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.incomplete",
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response": {
            "id": response_id,
            "status": "incomplete",
            "incomplete_details": {
                "reason": reason,
                "message": message,
            },
        },
    })
}

pub fn gemini_provider_core_response_metadata_event(
    sequence_number: u64,
    created_at: u64,
    response_id: &str,
    metadata: Value,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let metadata = serde_json::to_string(&metadata).expect("metadata serializes");
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::ResponseMetadata);
        input.sequence_number = sequence_number;
        input.created_at = created_at;
        input.response_id = Some(response_id);
        input.metadata = Some(&metadata);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.metadata",
        "sequence_number": sequence_number,
        "created_at": created_at,
        "response_id": response_id,
        "metadata": metadata,
    })
}

pub fn gemini_provider_core_output_item_added_event(sequence_number: u64, item: &Value) -> Value {
    #[cfg(feature = "mojo")]
    {
        let item = serde_json::to_string(item).expect("output item serializes");
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::OutputItemAdded);
        input.sequence_number = sequence_number;
        input.item = Some(&item);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.output_item.added",
        "sequence_number": sequence_number,
        "item": item,
    })
}

pub fn gemini_provider_core_output_item_done_event(
    sequence_number: u64,
    response_id: Option<&str>,
    item: &Value,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let item = serde_json::to_string(item).expect("output item serializes");
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::OutputItemDone);
        input.sequence_number = sequence_number;
        input.response_id = response_id;
        input.item = Some(&item);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    {
        let mut event = json!({
            "type": "response.output_item.done",
            "sequence_number": sequence_number,
            "item": item,
        });
        if let Some(response_id) = response_id {
            event["response_id"] = Value::String(response_id.to_string());
        }
        event
    }
}

pub fn gemini_provider_core_stream_function_call_arguments_delta_source(
    call_id: &str,
    name: &str,
    arguments: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let arguments = serde_json::from_str::<Value>(arguments).unwrap_or_else(|_| json!({}));
        let arguments = serde_json::to_string(&arguments).expect("arguments serialize");
        let mut input =
            GeminiResponseKernelInput::new(GeminiResponseKernelOperation::FunctionCallSource);
        input.call_id = Some(call_id);
        input.name = Some(name);
        input.arguments = Some(&arguments);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "functionCall": {
                        "id": call_id,
                        "name": name,
                        "args": serde_json::from_str::<Value>(arguments)
                            .unwrap_or_else(|_| json!({})),
                    }
                }]
            }
        }]
    })
}

pub fn gemini_provider_core_function_call_arguments_delta_event(
    sequence_number: u64,
    call_id: &str,
    arguments: &str,
) -> Value {
    #[cfg(feature = "mojo")]
    {
        let mut input = GeminiResponseKernelInput::new(
            GeminiResponseKernelOperation::FunctionCallArgumentsDelta,
        );
        input.sequence_number = sequence_number;
        input.call_id = Some(call_id);
        input.delta = Some(arguments);
        gemini_mojo_value(input)
    }
    #[cfg(not(feature = "mojo"))]
    json!({
        "type": "response.function_call_arguments.delta",
        "sequence_number": sequence_number,
        "call_id": call_id,
        "delta": arguments,
    })
}
