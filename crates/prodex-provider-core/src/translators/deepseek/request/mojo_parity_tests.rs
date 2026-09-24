use serde_json::{Value, json};

use super::{
    deepseek_common_request_body_from_responses_mojo,
    params::deepseek_insert_primitive_request_fields, params::deepseek_stop_from_request,
    params::deepseek_top_logprobs_from_request, params::deepseek_user_id_from_request,
};

fn rust_parameter_error(value: &Value) -> Option<String> {
    let mut fields = serde_json::Map::new();
    if let Err(error) = deepseek_insert_primitive_request_fields(value, &mut fields) {
        return Some(error);
    }
    if let Err(error) = deepseek_top_logprobs_from_request(value) {
        return Some(error);
    }
    if let Err(error) = deepseek_stop_from_request(value) {
        return Some(error);
    }
    deepseek_user_id_from_request(value).err()
}

#[test]
fn responses_request_parameter_plan_matches_rust_oracle_at_boundaries() {
    let too_many_stops = Value::Array(vec![json!("stop"); 17]);
    let too_long_user_id = "u".repeat(513);
    let cases = [
        json!({"input": "hello", "temperature": 0.2, "top_p": 1.0}),
        json!({"input": "hello", "temperature": "0.2"}),
        json!({"input": "hello", "top_p": false}),
        json!({"input": "hello", "max_output_tokens": 0}),
        json!({"input": "hello", "max_tokens": -1}),
        json!({"input": "hello", "max_completion_tokens": 1.0}),
        json!({"input": "hello", "logprobs": "true"}),
        json!({"input": "hello", "top_logprobs": 1.0, "logprobs": true}),
        json!({"input": "hello", "top_logprobs": 21, "logprobs": true}),
        json!({"input": "hello", "top_logprobs": 3, "logprobs": false}),
        json!({"input": "hello", "top_logprobs": 20, "logprobs": true}),
        json!({"input": "hello", "stop": 1}),
        json!({"input": "hello", "stop": ["a", 1]}),
        json!({"input": "hello", "stop": too_many_stops}),
        json!({"input": "hello", "stop_sequences": ["END"]}),
        json!({"input": "hello", "user_id": "bad!"}),
        json!({"input": "hello", "user_id": too_long_user_id}),
        json!({"input": "hello", "user": " \u{2003}user_1\u{00a0} "}),
        json!({
            "input": "hello",
            "temperature": "bad",
            "top_logprobs": 21,
            "stop": 1
        }),
    ];

    for value in cases {
        let expected = rust_parameter_error(&value);
        let object = value.as_object().expect("request fixture is an object");
        let actual = deepseek_common_request_body_from_responses_mojo(object, &value)
            .map(|_| ())
            .err();
        assert_eq!(actual, expected, "request: {value}");
    }
}
