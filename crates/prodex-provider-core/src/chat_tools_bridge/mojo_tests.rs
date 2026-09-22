use super::*;
use serde_json::{Value, json};

fn compare(value: &Value) {
    assert_eq!(
        provider_core_chat_tools_from_responses_request(value),
        tools::provider_core_chat_tools_from_responses_request(value),
        "tools: {value}"
    );
    for thinking in [false, true] {
        assert_eq!(
            provider_core_chat_tool_choice_from_responses_request(value, thinking),
            tool_choice::provider_core_chat_tool_choice_from_responses_request(value, thinking),
            "choice {thinking}: {value}"
        );
    }
    assert_eq!(
        provider_core_chat_web_search_options_from_responses_request(value),
        web_search::provider_core_chat_web_search_options_from_responses_request(value),
        "web: {value}"
    );
    let raw = serde_json::to_vec(value).unwrap();
    assert_eq!(
        provider_core_chat_request_body_without_web_search_options(&raw),
        web_search::provider_core_chat_request_body_without_web_search_options(&raw),
        "strip: {value}"
    );
}

const NAMES: &[&str] = &[
    "",
    " ",
    "apply_patch",
    "tool",
    "mcp__server",
    "_tool",
    "a__b",
    "server__",
    "get-item",
    "a🦀b",
    "東京",
    "\u{001c}name",
    "\u{2003}name\u{00a0}",
    "nul\0key",
    "quoted\"name",
    "back\\slash",
    "mcp__x",
    " mcp__x ",
    "__",
    "123",
    "
\r	",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self, max: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 32) as usize % max
    }
    fn name(&mut self) -> &'static str {
        NAMES[self.next(NAMES.len())]
    }
    fn scalar(&mut self) -> Value {
        match self.next(7) {
            0 => Value::Null,
            1 => json!(false),
            2 => json!(true),
            3 => json!(self.next(1000)),
            4 => json!(-2.5e-10),
            _ => json!(self.name()),
        }
    }
    fn shape(&mut self) -> Value {
        match self.next(5) {
            0 => self.scalar(),
            1 => json!([self.scalar(), self.scalar()]),
            _ => json!({"type":"object", "properties":{self.name(): self.scalar()},
                "extra":self.scalar(), "required":[self.name()]}),
        }
    }
    fn function(&mut self) -> Value {
        let mut value = json!({"type":"function"});
        for key in ["name", "description"] {
            if self.next(4) > 0 {
                value[key] = json!(self.name());
            }
        }
        for key in [
            "parameters",
            "parametersJsonSchema",
            "input_schema",
            "schema",
            "strict",
        ] {
            if self.next(4) > 0 {
                value[key] = self.shape();
            }
        }
        value
    }
    fn tool(&mut self) -> Value {
        let kinds = [
            "function",
            "custom",
            "namespace",
            "tool_search",
            "mcp",
            "mcp_toolset",
            "mcp_provider",
            "web_search",
            "web_search_preview",
            "web_search_preview_v2",
            "",
            "other",
        ];
        let mut value = self.function();
        value["type"] = json!(kinds[self.next(kinds.len())]);
        if self.next(2) == 0 {
            value["function"] = self.function();
        }
        if self.next(3) == 0 {
            value["format"] = self.shape();
        }
        value["tools"] = json!([self.function(), self.function(), self.scalar()]);
        for key in [
            "mcp_server_name",
            "server_label",
            "server_name",
            "search_context_size",
            "context_size",
        ] {
            if self.next(3) > 0 {
                value[key] = if self.next(4) == 0 {
                    self.scalar()
                } else {
                    json!(self.name())
                };
            }
        }
        if self.next(2) == 0 {
            value["search_context_size"] =
                json!(["low", "medium", "high", "invalid"][self.next(4)]);
        }
        value["allowed_tools"] = json!([self.name(), self.name(), self.name(), self.scalar()]);
        value["configs"] = json!({self.name(): {"enabled":self.scalar()}, self.name(): {"enabled":self.scalar()}, self.name():self.scalar()});
        if self.next(2) == 0 {
            value["default_config"] = json!({"enabled":self.scalar()});
        }
        for key in [
            "allowed_domains",
            "blocked_domains",
            "max_uses",
            "user_location",
            "location",
        ] {
            if self.next(3) > 0 {
                value[key] = self.shape();
            }
        }
        value
    }
    fn choice(&mut self) -> Value {
        let kinds = ["function", "mcp", "mcp_toolset", "other", ""];
        if self.next(3) == 0 {
            return json!(["auto", "none", "required", "other", ""][self.next(5)]);
        }
        let mut value = json!({"type":kinds[self.next(kinds.len())], "function":{"name":self.name(), "namespace":self.name()}});
        for key in [
            "name",
            "namespace",
            "server_label",
            "mcp_server_name",
            "server_name",
        ] {
            if self.next(3) > 0 {
                value[key] = if self.next(3) == 0 {
                    self.scalar()
                } else {
                    json!(self.name())
                };
            }
        }
        value
    }
}

#[test]
fn complete_tool_shapes_match_five_thousand_seeded_json_trees() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let mut rng = Rng(0x918e_ab33_e824_f712);
    for _ in 0..5_000 {
        let tools = (0..rng.next(9)).map(|_| rng.tool()).collect::<Vec<_>>();
        let value = json!({"tools":tools, "tool_choice":rng.choice(), "web_search_options":rng.shape(),
            "metadata":{"保持":rng.scalar()}, "input":"unrelated request content"});
        compare(&value);
    }
    for value in [
        Value::Null,
        json!(true),
        json!([]),
        json!({}),
        json!({"tools":null}),
        json!({"tools":[1,null,"text"]}),
    ] {
        compare(&value);
    }
}

#[test]
fn namespace_names_match_unicode_and_mcp_prefix_oracle() {
    for namespace in NAMES {
        for name in NAMES {
            assert_eq!(
                provider_core_flatten_namespace_tool_name(namespace, name),
                util::provider_core_flatten_namespace_tool_name(namespace, name),
                "{namespace:?} {name:?}"
            );
        }
    }
}

#[test]
fn mcp_sort_dedup_and_custom_format_are_exact() {
    compare(&json!({"tools":[
        {"type":"mcp", "server_label":" git tools ", "allowed_tools":[" z ","a","a","mcp__ready", "nul\0key"],
            "configs":{"b":{"enabled":true},"a":{"enabled":true},"no":{"enabled":false},"!invalid!":{}}, "default_config":{"enabled":false}},
        {"type":"custom", "name":"apply_patch", "format":{"type":"grammar", "unicode":"🦀", "text":"
    \"\\"}},
        {"type":"custom", "name":"other", "description":"custom\0description", "format":null},
        {"type":"namespace", "name":"mcp__s", "tools":[{"type":"function","name":"fn","parameters":null,"input_schema":{}}]}
    ]}));
}

#[test]
fn measured_output_handles_expansion_without_an_arbitrary_tool_cap() {
    let tools = (0..1024).map(|index| json!({"type":"custom", "name":format!("tool_{index}"), "format":{"type":"grammar"}})).collect::<Vec<_>>();
    compare(&json!({"tools":tools}));
    let names = (0..2048)
        .rev()
        .map(|i| format!("tool_{i:05}"))
        .collect::<Vec<_>>();
    compare(
        &json!({"tools":[{"type":"mcp_toolset","server_name":"server","allowed_tools":names}]}),
    );
}

/// Manual complete-boundary benchmark, deliberately outside correctness gates.
#[test]
#[ignore = "run with --release --ignored --nocapture for boundary timing"]
fn complete_provider_tool_boundary_benchmark() {
    use std::hint::black_box;
    use std::time::Instant;
    fn median_ns(mut call: impl FnMut()) -> u128 {
        let mut samples = Vec::new();
        for _ in 0..9 {
            let started = Instant::now();
            for _ in 0..25 {
                call();
            }
            samples.push(started.elapsed().as_nanos() / 25);
        }
        samples.sort_unstable();
        samples[samples.len() / 2]
    }
    for count in [0, 8, 64] {
        let tools = (0..count).map(|index| json!({"type":"function", "name":format!("tool_{index}"),
            "description":"Provider tool boundary benchmark.",
            "parameters":{"type":"object", "properties":{"name":{"type":"string"},"count":{"type":"integer"}},"required":["name"]},"strict":true})).collect::<Vec<_>>();
        let input = json!({"tools":tools});
        let rust = median_ns(|| {
            black_box(tools::provider_core_chat_tools_from_responses_request(
                black_box(&input),
            ));
        });
        let mojo = median_ns(|| {
            black_box(provider_core_chat_tools_from_responses_request(black_box(
                &input,
            )));
        });
        println!("tools={count} rust_ns={rust} mojo_boundary_ns={mojo}");
    }
}
