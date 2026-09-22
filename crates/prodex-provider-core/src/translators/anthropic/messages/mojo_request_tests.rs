use super::*;
use serde_json::{Map, Value, json};

fn assert_parity(chat: Value) {
    let body = serde_json::to_vec(&chat).expect("fixture serializes");
    let input = ProviderTransformInput::new(ProviderEndpoint::Responses, body);
    let actual = translate_chat_request_to_anthropic(input.clone());
    let expected = translate_chat_request_to_anthropic_rust(input);
    assert_eq!(actual.provider, expected.provider);
    assert_eq!(actual.endpoint, expected.endpoint);
    assert_eq!(actual.from_format, expected.from_format);
    assert_eq!(actual.to_format, expected.to_format);
    assert_eq!(actual.headers, expected.headers);
    assert_eq!(actual.metadata, expected.metadata);
    assert_eq!(actual.loss, expected.loss, "chat={chat}");
    match (actual.body, expected.body) {
        (Some(actual), Some(expected)) => {
            let actual: Value = serde_json::from_slice(&actual).expect("Mojo body JSON");
            let expected: Value = serde_json::from_slice(&expected).expect("Rust body JSON");
            assert_eq!(actual, expected, "chat={chat}");
        }
        (None, None) => {}
        (actual, expected) => {
            panic!("body presence differs chat={chat}: actual={actual:?} expected={expected:?}")
        }
    }
}

#[test]
fn complete_anthropic_chat_request_matches_edge_contracts() {
    let fixtures = [
        json!({"messages":[{"role":"user","content":"hello"}]}),
        json!({"messages":[
            {"role":"system","content":"one"},
            {"role":"developer","content":"two"},
            {"role":"user","content":"hello"}
        ]}),
        json!({"messages":[
            {"role":"user","content":"before"},
            {"role":"tool","tool_call_id":"c1","content":"result"},
            {"role":"user","content":"after"}
        ]}),
        json!({"messages":[{"role":"assistant","content":"thinking","namespace":"agents","tool_calls":[{
            "id":"c1","type":"function","function":{"name":"run","arguments":"{\"x\":1}"}
        }]}]}),
        json!({"messages":[{"role":"assistant","tool_calls":[{
            "function":{"name":"a.b","arguments":"[]"}
        }]}]}),
        json!({"messages":[{"role":"assistant","tool_calls":[{
            "function":{"name":"a.b","arguments":"{bad"}
        }]}]}),
        json!({"messages":[{"role":"user","content":"x"}],"model":17,"max_tokens":"many","stream":"yes","temperature":0.2,"top_p":0.8}),
        json!({"messages":[{"role":"user","content":"x"}],"stop":"done"}),
        json!({"messages":[{"role":"user","content":"x"}],"stop":["a","b"]}),
        json!({"messages":[{"role":"user","content":"x"}],"stop":17}),
        json!({"messages":[{"role":"user","content":"x"}],"tools":[{
            "type":"function","function":{"name":"mcp.tools.read","namespace":"ns","description":"d","parameters":{"type":"object"}}
        }]}),
        json!({"messages":[{"role":"user","content":"x"}],"web_search_options":{"search_context_size":"high","allowed_domains":["example.com"],"max_uses":2}}),
        json!({"messages":[{"role":"user","content":"x"}],"tools":[{"name":"f"}],"web_search_options":{"search_context_size":"low"},"tool_choice":"none"}),
        json!({"messages":[{"role":"user","content":"x"}],"tool_choice":"auto"}),
        json!({"messages":[{"role":"user","content":"x"}],"tool_choice":"required"}),
        json!({"messages":[{"role":"user","content":"x"}],"tool_choice":{"type":"function","namespace":"ns","name":"f"}}),
        json!({"messages":[{"role":"user","content":"x"}],"parallel_tool_calls":false}),
        json!({"messages":[{"role":"user","content":"x"}],"unknown":true}),
        json!({"messages":[]}),
        json!({"messages":"bad"}),
        json!([]),
    ];
    for fixture in fixtures {
        assert_parity(fixture);
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
    fn pick(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn generated_chat(rng: &mut Lcg, index: usize) -> Value {
    let mut chat = Map::new();
    let mut messages = Vec::new();
    if rng.pick(4) == 0 {
        messages.push(json!({"role":"system","content":format!("system-{index}")}));
    }
    messages.push(match rng.pick(4) {
        0 => json!({"role":"user","content":format!("hello-{index}-東京")}),
        1 => json!({"role":"assistant","content":format!("answer-{index}")}),
        2 => json!({"role":"assistant","namespace":"agents","tool_calls":[{
            "id":format!("call-{index}"),"function":{"name":"run","arguments":format!("{{\"index\":{index}}}")}
        }]}),
        _ => json!({"role":"tool","tool_call_id":format!("call-{index}"),"content":format!("out-{index}")}),
    });
    if rng.pick(3) == 0 {
        messages.push(json!({"role":"user","content":format!("tail-{index}")}));
    }
    chat.insert("messages".into(), Value::Array(messages));
    if rng.pick(3) == 0 {
        chat.insert("model".into(), json!(format!("model-{}", rng.pick(5))));
    }
    if rng.pick(3) == 0 {
        chat.insert("max_tokens".into(), json!(1 + rng.pick(8192)));
    }
    if rng.pick(4) == 0 {
        chat.insert("stream".into(), json!(rng.pick(2) == 0));
    }
    if rng.pick(5) == 0 {
        chat.insert("temperature".into(), json!((rng.pick(20) as f64) / 10.0));
    }
    if rng.pick(5) == 0 {
        chat.insert("top_p".into(), json!((rng.pick(10) as f64) / 10.0));
    }
    if rng.pick(6) == 0 {
        chat.insert("stop".into(), json!(["stop"]));
    }
    if rng.pick(6) == 0 {
        chat.insert(
            "tools".into(),
            json!([{"type":"function","function":{"name":"f","parameters":{"type":"object"}}}]),
        );
    }
    if rng.pick(8) == 0 {
        let context_size = ["low", "medium", "high"][rng.pick(3) as usize];
        chat.insert(
            "web_search_options".into(),
            json!({"search_context_size": context_size, "max_uses": 1 + rng.pick(4)}),
        );
    }
    match rng.pick(20) {
        0 => {
            chat.insert("parallel_tool_calls".into(), json!(false));
        }
        1 => {
            chat.insert("unknown".into(), json!(true));
        }
        2 => {
            chat.insert("stop".into(), json!(17));
        }
        3 => {
            chat.insert("tools".into(), json!("bad"));
        }
        4 => {
            chat.insert("tool_choice".into(), json!("unsupported"));
        }
        5 => {
            chat.insert(
                "web_search_options".into(),
                json!({"allowed_domains":["a"],"blocked_domains":["b"]}),
            );
        }
        _ => {}
    }
    Value::Object(chat)
}

#[test]
fn complete_anthropic_chat_request_matches_five_thousand_generated_chats() {
    let mut rng = Lcg(0x616e7468726f7069);
    for index in 0..5_000 {
        assert_parity(generated_chat(&mut rng, index));
    }
}
