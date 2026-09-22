use super::*;
use serde_json::{Value, json};

struct Rng(u64);
impl Rng {
    fn next(&mut self, max: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 32) as usize % max
    }
    fn id(&mut self) -> &'static str {
        [
            "", "a", "b", "c", " a ", "🔥", "nul\0id", "\u{001c}", "\u{2003}",
        ][self.next(9)]
    }
    fn content(&mut self) -> Value {
        match self.next(12) {
            0 => Value::Null,
            1 => json!(true),
            2 => json!(false),
            3 => json!(0),
            4 => json!([]),
            5 => json!({}),
            6 => json!([null]),
            7 => json!({"x":null}),
            _ => json!(self.id()),
        }
    }
    fn metadata(&mut self) -> Value {
        match self.next(3) {
            0 => self.content(),
            _ => {
                json!({"a":self.content(), "b":{"nested":self.content()}, self.id():self.content()})
            }
        }
    }
    fn message(&mut self) -> Value {
        if self.next(10) == 0 {
            return self.content();
        }
        let mut message =
            json!({"role":(["assistant","tool","user","system","other",""][self.next(6)])});
        if self.next(3) > 0 {
            message["content"] = self.content();
        }
        if self.next(3) > 0 {
            message["reasoning_content"] = self.content();
        }
        if self.next(2) > 0 {
            message["tool_call_id"] = json!(self.id());
        }
        if self.next(2) > 0 {
            let calls = (0..self.next(6)).map(|_| {
                if self.next(4) == 0 { self.content() }
                else { json!({"id":self.id(), "type":"function", "function":{"name":"test_tool","arguments":"{}"}}) }
            }).collect::<Vec<_>>();
            message["tool_calls"] = json!(calls);
        } else if self.next(3) == 0 {
            message["tool_calls"] = self.content();
        }
        message["metadata"] = self.metadata();
        message
    }
}

fn compare_messages(input: &[Value]) {
    let mut actual = input.to_vec();
    let mut expected = input.to_vec();
    deepseek_provider_core_normalize_thinking_tool_call_messages(&mut actual);
    deepseek_provider_core_normalize_thinking_tool_call_messages_rust(&mut expected);
    assert_eq!(actual, expected, "thinking: {input:?}");
    for message in input.iter().take(4) {
        assert_eq!(
            deepseek_provider_core_normalize_assistant_tool_call_content(message.clone()),
            deepseek_provider_core_normalize_assistant_tool_call_content_rust(message.clone()),
            "content: {message}"
        );
    }
    let mut actual = input.to_vec();
    let mut expected = input.to_vec();
    deepseek_provider_core_repair_tool_call_adjacency(&mut actual);
    adjacency::deepseek_provider_core_repair_tool_call_adjacency(&mut expected);
    assert_eq!(actual, expected, "adjacency: {input:?}");
}

#[test]
fn message_plans_match_ten_thousand_seeded_histories() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let mut rng = Rng(0x27bf_194c_967a_c15d);
    for _ in 0..10_000 {
        let input = (0..rng.next(25)).map(|_| rng.message()).collect::<Vec<_>>();
        compare_messages(&input);
        let mut response = if rng.next(3) == 0 {
            rng.content()
        } else {
            json!({"id":"test_response", "metadata":rng.metadata(), "extra":rng.content()})
        };
        if rng.next(2) == 0
            && let Some(object) = response.as_object_mut()
        {
            object.remove("metadata");
        }
        let metadata = if rng.next(3) == 0 {
            None
        } else {
            Some(rng.metadata())
        };
        let mut expected = response.clone();
        deepseek_provider_core_merge_response_metadata(&mut response, metadata.clone());
        deepseek_provider_core_merge_response_metadata_rust(&mut expected, metadata);
        assert_eq!(response, expected, "metadata merge");
    }
}

#[test]
fn adjacency_preserves_first_output_and_global_single_emission() {
    compare_messages(&[
        json!({"role":"tool","tool_call_id":"a", "content":"first"}),
        json!({"role":"tool","tool_call_id":"a", "content":"duplicate"}),
        json!({"role":"assistant","tool_calls":[{"id":"a"},{"id":"a"},{"id":"missing"}]}),
        json!({"role":"assistant","tool_calls":[{"id":"a"}],"content":"keep prose"}),
        json!({"role":"user","tool_calls":[{"id":"b"}]}),
        json!({"role":"tool","tool_call_id":"b","content":"second result"}),
    ]);
    compare_messages(&[
        json!({"role":"tool","tool_call_id":" ","content":"not a valid output ID"}),
        json!({"role":"assistant","tool_calls":[{"id":"a"}],"content":false}),
        json!({"role":"assistant","tool_calls":[{"id":"a"}],"reasoning_content":[null]}),
        json!({"role":"assistant","tool_calls":[{"id":"a"}],"content":"\u{001c}"}),
        json!({"role":"assistant","tool_calls":[{"id":"a"}],"content":"\u{2003}"}),
    ]);
}

#[test]
fn metadata_merge_keeps_scalar_when_incoming_value_is_object() {
    let mut response = json!({"metadata":{"scalar":false,"object":{"old":1,"nested":{"a":1}}}});
    let incoming = json!({"scalar":{"new":2},"object":{"new":2,"nested":{"b":2}},"missing":{}});
    deepseek_provider_core_merge_response_metadata(&mut response, Some(incoming));
    assert_eq!(
        response,
        json!({"metadata":{"scalar":false,"object":{"old":1,"new":2,"nested":{"b":2}},"missing":{}}})
    );
}

#[test]
fn adjacency_handles_two_thousand_unique_out_of_order_calls() {
    let mut input = (0..2048)
        .map(|index| json!({"role":"assistant", "tool_calls":[{"id":format!("call_{index:05}")}]}))
        .collect::<Vec<_>>();
    input.extend((0..2048).rev().map(|index| json!({"role":"tool", "tool_call_id":format!("call_{index:05}"), "content":format!("output {index}")})));
    compare_messages(&input);
}

/// Explicit whole-boundary measurement, never an ignored correctness check.
#[test]
#[ignore = "manual release-profile message-boundary benchmark"]
fn complete_message_boundary_benchmark() {
    use std::hint::black_box;
    use std::time::Instant;
    fn median(mut call: impl FnMut()) -> u128 {
        let mut samples = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..10 {
                call();
            }
            samples.push(start.elapsed().as_nanos() / 10);
        }
        samples.sort_unstable();
        samples[samples.len() / 2]
    }
    for (pairs, content_bytes) in [(8, 64), (64, 64), (64, 4096)] {
        let mut input = Vec::new();
        for index in 0..pairs {
            input.push(json!({"role":"assistant","tool_calls":[{"id":format!("call_{index}"), "function":{"name":"test_tool","arguments":"{}"}}]}));
            input.push(json!({"role":"tool","tool_call_id":format!("call_{index}"),"content":"x".repeat(content_bytes)}));
        }
        let rust = median(|| {
            let mut messages = input.clone();
            adjacency::deepseek_provider_core_repair_tool_call_adjacency(black_box(&mut messages));
            black_box(messages);
        });
        let mojo = median(|| {
            let mut messages = input.clone();
            deepseek_provider_core_repair_tool_call_adjacency(black_box(&mut messages));
            black_box(messages);
        });
        println!(
            "pairs={pairs} content_bytes={content_bytes} rust_ns={rust} mojo_boundary_ns={mojo}"
        );
    }
}
