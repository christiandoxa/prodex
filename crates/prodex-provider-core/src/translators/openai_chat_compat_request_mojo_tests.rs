use super::*;
use serde_json::{Map, Value, json};

fn assert_request_parity(request: Value, input_model: Option<&str>, default_model: &str) {
    let body = serde_json::to_vec(&request).expect("fixture serializes");
    let mut input = ProviderTransformInput::new(ProviderEndpoint::Responses, body);
    input.model = input_model.map(str::to_owned);
    let actual = translate_responses_request_to_chat(provider(), input.clone(), default_model);
    let expected = translate_responses_request_to_chat_rust(provider(), input, default_model);

    assert_eq!(actual.provider, expected.provider);
    assert_eq!(actual.endpoint, expected.endpoint);
    assert_eq!(actual.from_format, expected.from_format);
    assert_eq!(actual.to_format, expected.to_format);
    assert_eq!(actual.headers, expected.headers);
    assert_eq!(actual.metadata, expected.metadata);
    assert_eq!(actual.loss, expected.loss, "request={request}");

    match (actual.body, expected.body) {
        (Some(actual), Some(expected)) => {
            let actual: Value = serde_json::from_slice(&actual).expect("Mojo body is JSON");
            let expected: Value = serde_json::from_slice(&expected).expect("Rust body is JSON");
            assert_eq!(actual, expected, "request={request}");
        }
        (None, None) => {}
        (actual, expected) => panic!(
            "body presence differs for request={request}: actual={actual:?} expected={expected:?}"
        ),
    }
}

fn provider() -> ProviderId {
    ProviderId::Anthropic
}

#[test]
fn complete_openai_chat_request_matches_edge_contracts() {
    let fixtures = [
        json!({"input": "hello"}),
        json!({"instructions": "system", "input": "hello", "model": "request-model"}),
        json!({"instructions": "", "input": " "}),
        json!({"input": [{"type":"message","role":"assistant","content":"hello"}]}),
        json!({"input": [{"type":"message","role":"assistant","content":"","text":"blocked"}]}),
        json!({"input": [{"type":"message","role":"assistant","content":[{"type":"input_text","text":"a"},{"type":"text","content":"b"}]}]}),
        json!({"input": [{"type":"input_text","text":" \t "},{"type":"output_text","text":"assistant"}]}),
        json!({"input": [{"type":"function_call","call_id":"c1","namespace":"agents","name":"run","arguments":{"x":1}}]}),
        json!({"input": [{"type":"function_call","call_id":17,"id":"fallback","name":"run","arguments":true}]}),
        json!({"input": [{"type":"function_call","name":null,"tool_name":"blocked","arguments":"{}"}]}),
        json!({"input": [{"type":"function_call_output","call_id":"c1","output":{"ok":true}}]}),
        json!({"input": [{"type":"function_call_output","call_id":17,"id":"blocked","output":"x"}]}),
        json!({
            "input":"hello",
            "temperature":0.2,
            "top_p":0.9,
            "presence_penalty":0.1,
            "frequency_penalty":0.3,
            "seed":42,
            "max_completion_tokens":77,
            "max_output_tokens":88,
            "max_tokens":99,
            "tools":[{"type":"function","name":"f"}],
            "tool_choice":"auto",
            "parallel_tool_calls":true,
            "user":"u",
            "stream":true
        }),
        json!({"messages":[],"input":"x"}),
        json!({"response_format":{"type":"json_object"},"input":"x"}),
        json!({"reasoning":{"effort":"high"},"input":"x"}),
        json!({"previous_response_id":"resp_1","input":"x"}),
        json!({"text":{"format":{"type":"json_schema"}},"input":"x"}),
        json!({"n":2,"input":"x"}),
        json!({"n":2.0,"input":"x"}),
        json!({"metadata":{},"input":"x"}),
        json!({"safety_identifier":"s","input":"x"}),
        json!({"web_search_options":{},"input":"x"}),
        json!({"tools":[{"type":"custom","name":"x"}],"input":"x"}),
        json!({"tool_choice":{"type":"mcp","name":"x"},"input":"x"}),
        json!({"parallel_tool_calls":false,"input":"x"}),
        json!({"logprobs":true,"input":"x"}),
        json!({"top_logprobs":2,"input":"x"}),
        json!({"stop_sequences":["x"],"input":"x"}),
        json!({"input":[{"type":"custom_tool_call","name":"x"}]}),
        json!({"input":[{"type":"input_image","image_url":"data:image/png;base64,AA=="}]}),
        json!({"input":[]}),
        json!({"input":null}),
    ];
    for fixture in fixtures {
        assert_request_parity(fixture, Some("input-model"), "default-model");
    }
}

#[derive(Clone, Copy)]
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn pick(&mut self, count: u64) -> u64 {
        self.next() % count
    }
}

fn generated_request(rng: &mut Lcg, index: usize) -> Value {
    let text = format!("case-{index}-東京-{}", rng.pick(10_000));
    let mut request = Map::new();
    let input = match rng.pick(7) {
        0 => Value::String(text.clone()),
        1 => {
            json!([{"type":"message","role": if rng.pick(2)==0 {"user"} else {"assistant"},"content":text}])
        }
        2 => json!([{"type":"message","content":[
            {"type":"input_text","text":text},
            {"type":"text","content":"tail"}
        ]}]),
        3 => {
            json!([{"type":"function_call","call_id":format!("call-{index}"),"namespace":"functions","name":"exec_command","arguments":{"cmd":"pwd"}}])
        }
        4 => {
            json!([{"type":"function_call_output","call_id":format!("call-{index}"),"output":{"ok":true,"index":index}}])
        }
        5 => json!([{"type":"input_text","text":text},{"type":"output_text","text":"done"}]),
        _ => json!({"type":"message","role":"user","text":text}),
    };
    request.insert("input".into(), input);

    if rng.pick(3) == 0 {
        request.insert(
            "instructions".into(),
            Value::String(format!("system-{index}")),
        );
    }
    if rng.pick(4) == 0 {
        request.insert(
            "model".into(),
            Value::String(format!("model-{}", rng.pick(5))),
        );
    }
    if rng.pick(2) == 0 {
        request.insert("stream".into(), Value::Bool(rng.pick(2) == 0));
    }
    if rng.pick(3) == 0 {
        request.insert("temperature".into(), json!((rng.pick(20) as f64) / 10.0));
    }
    if rng.pick(4) == 0 {
        request.insert("top_p".into(), json!((rng.pick(10) as f64) / 10.0));
    }
    if rng.pick(5) == 0 {
        request.insert("max_output_tokens".into(), json!(1 + rng.pick(4096)));
    }
    if rng.pick(6) == 0 {
        request.insert(
            "tools".into(),
            json!([{"type":"function","name":format!("tool_{index}"),"parameters":{"type":"object"}}]),
        );
    }
    if rng.pick(6) == 0 {
        request.insert("tool_choice".into(), Value::String("auto".into()));
    }

    match rng.pick(18) {
        0 => {
            request.insert("messages".into(), json!([]));
        }
        1 => {
            request.insert("response_format".into(), json!({"type":"json_object"}));
        }
        2 => {
            request.insert("reasoning".into(), json!({"effort":"high"}));
        }
        3 => {
            request.insert("previous_response_id".into(), json!("resp_prev"));
        }
        4 => {
            request.insert("metadata".into(), json!({"case":index}));
        }
        5 => {
            request.insert("safety_identifier".into(), json!("id"));
        }
        6 => {
            request.insert("web_search_options".into(), json!({}));
        }
        7 => {
            request.insert("parallel_tool_calls".into(), json!(false));
        }
        8 => {
            request.insert("logprobs".into(), json!(true));
        }
        9 => {
            request.insert("stop_sequences".into(), json!(["x"]));
        }
        10 => {
            request.insert("n".into(), json!(2));
        }
        11 => {
            request.insert("text".into(), json!({"format":{"type":"text"}}));
        }
        _ => {}
    }
    Value::Object(request)
}

#[test]
fn complete_openai_chat_request_matches_five_thousand_generated_requests() {
    let mut rng = Lcg(0x6f70656e61695f35);
    for index in 0..5_000 {
        let request = generated_request(&mut rng, index);
        let input_model = (rng.pick(3) == 0).then_some("caller-model");
        assert_request_parity(request, input_model, "default-model");
    }
}
