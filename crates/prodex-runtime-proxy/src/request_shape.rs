use serde::{
    Deserializer, Serialize, Serializer,
    de::{DeserializeSeed, MapAccess, SeqAccess, Visitor},
    ser::SerializeSeq,
};
use serde_json::{Map, Value};

#[derive(Clone, Copy)]
enum Projection {
    Root,
    Metadata,
    Input,
    Item,
    String,
    Ignore,
}

impl<'de> DeserializeSeed<'de> for Projection {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Projection {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(if matches!(self, Projection::String) {
            Value::String(value.to_owned())
        } else {
            Value::Null
        })
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        if matches!(self, Projection::Input) {
            let mut items = Vec::new();
            while let Some(item) = sequence.next_element_seed(Projection::Item)? {
                items.push(item);
            }
            Ok(Value::Array(items))
        } else {
            while sequence.next_element_seed(Projection::Ignore)?.is_some() {}
            Ok(Value::Null)
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut selected = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let projection = match (self, key.as_str()) {
                (Projection::Root, "input") => Projection::Input,
                (Projection::Root, "client_metadata") => Projection::Metadata,
                (
                    Projection::Root,
                    "previous_response_id"
                    | "session_id"
                    | "prompt_cache_key"
                    | "x-codex-turn-state"
                    | "turn_id"
                    | "thread_id"
                    | "window_id",
                )
                | (
                    Projection::Metadata,
                    "session_id" | "x-codex-turn-state" | "turn_id" | "thread_id"
                    | "x-codex-window-id",
                )
                | (Projection::Item, "type" | "role" | "call_id") => Projection::String,
                _ => Projection::Ignore,
            };
            let value = map.next_value_seed(projection)?;
            if !matches!(projection, Projection::Ignore) {
                selected.insert(key, value);
            }
        }
        Ok(
            if matches!(
                self,
                Projection::Root | Projection::Metadata | Projection::Item
            ) {
                Value::Object(selected)
            } else {
                Value::Null
            },
        )
    }
}

pub(super) fn runtime_request_body_plan(body: &[u8]) -> Option<super::RuntimeRequestSemanticPlan> {
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    let projection = Projection::Root.deserialize(&mut deserializer).ok()?;
    deserializer.end().ok()?;
    projection.as_object()?;
    Some(super::runtime_request_semantic_plan(&projection))
}

#[derive(Serialize)]
struct RequestShape<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<InputShapes<'a>>,
}

#[derive(Serialize)]
struct InputShape<'a> {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    call_id: Option<&'a str>,
}

struct InputShapes<'a>(&'a [Value]);

impl Serialize for InputShapes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for item in self.0 {
            sequence.serialize_element(&InputShape {
                kind: item.get("type").and_then(Value::as_str),
                role: item.get("role").and_then(Value::as_str),
                call_id: item.get("call_id").and_then(Value::as_str),
            })?;
        }
        sequence.end()
    }
}

// Mojo's request-shape policy inspects only these fields, not message content.
pub(super) fn runtime_request_shape_json(value: &Value) -> String {
    if !value.is_object() {
        return serde_json::to_string(value).expect("runtime request JSON serializes");
    }
    let shape = RequestShape {
        previous_response_id: value.get("previous_response_id").and_then(Value::as_str),
        input: value
            .get("input")
            .and_then(Value::as_array)
            .map(|items| InputShapes(items)),
    };
    serde_json::to_string(&shape).expect("runtime request shape serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_mojo_core::json::runtime_proxy_request_shape;
    use serde_json::json;

    #[test]
    fn body_projection_preserves_metadata_and_mojo_decisions() {
        let cases = [
            r#"{"previous_response_id":"old","previous_response_id":"resp-1","session_id":null,"client_metadata":{"session_id":"nested","turn_id":"turn","x-codex-window-id":"s:4"},"input":[{"role":"user","content":[{"type":"input_image","image_url":"data:image/png;base64,AAAA"}]},{"type":"function_call","call_id":"c"},{"role":"assistant"}]}"#,
            r#"{"previous_response_id":"resp-1","client_metadata":{"session_id":"nested"},"input":[{"type":"function_call_output","call_id":"call-1","output":"secret"}]}"#,
            r#"{"previous_response_id":42,"prompt_cache_key":false,"input":[null,1,"x",[],{"role":{},"type":3,"call_id":true}]}"#,
            r#"{"session_id":"  ","client_metadata":{"session_id":"nested","turn_id":"nested"},"turn_id":null,"input":null}"#,
            r#"{"previous_response_id":"  ","input":[{"type":"compaction"},{"role":"user"}]}"#,
            r#"{"\u0070revious_response_id":"resp-2","input":[{"type":"custom_tool_call_output","call_id":"\\u0061"}]}"#,
            r#"{}"#,
        ];
        for text in cases {
            let value: Value = serde_json::from_str(text).expect("fixture");
            assert_eq!(
                runtime_request_body_plan(text.as_bytes()),
                Some(super::super::runtime_request_semantic_plan(&value)),
                "{text}"
            );
        }
        assert!(runtime_request_body_plan(br#"{"input":["unfinished"#).is_none());
        for text in [
            br#"{"input":[{"content":1e999}],"previous_response_id":"resp-1"}"#.as_slice(),
            br#"{"input":[{"content":"\ud800"}],"previous_response_id":"resp-1"}"#,
        ] {
            assert_eq!(
                runtime_request_body_plan(text).is_some(),
                serde_json::from_slice::<Value>(text).is_ok(),
            );
        }

        for depth in [0, 1, 64, 125, 126, 127, 128, 129, 200] {
            let text = format!(
                "{{\"input\":[],\"ignored\":{}0{}}}",
                "[".repeat(depth),
                "]".repeat(depth)
            );
            let value = serde_json::from_str::<Value>(&text);
            assert_eq!(
                runtime_request_body_plan(text.as_bytes()),
                value
                    .ok()
                    .map(|value| super::super::runtime_request_semantic_plan(&value)),
                "nesting depth {depth}"
            );
        }
    }

    #[test]
    fn body_projection_keeps_affinity_fields_after_large_image() {
        let image = "A".repeat(4 * 1024 * 1024);
        let text = format!(
            r#"{{"input":[{{"role":"user","content":[{{"type":"input_image","image_url":"data:image/png;base64,{image}"}}]}},{{"type":"function_call_output","call_id":"call-1","output":"done"}}],"previous_response_id":"resp-1","client_metadata":{{"session_id":"session-1"}}}}"#
        );
        let value: Value = serde_json::from_str(&text).expect("valid request");
        let plan = runtime_request_body_plan(text.as_bytes()).expect("request projection");
        assert_eq!(plan, super::super::runtime_request_semantic_plan(&value));
        assert_eq!(plan.previous_response_id.as_deref(), Some("resp-1"));
        assert_eq!(plan.session_id.as_deref(), Some("session-1"));
        assert!(plan.requires_previous_response_affinity);
    }

    #[test]
    fn projected_request_shape_preserves_mojo_decisions_without_image_payload() {
        let image = "A".repeat(1024 * 1024);
        let inputs = [
            json!(null),
            json!([]),
            json!([null, 5, "text"]),
            json!([{"role":"user", "content":image}]),
            json!([{"type":"function_call_output", "call_id":"call-1", "output":"x"}]),
            json!([{"type":"compaction"}, {"role":"user"}]),
            json!([{"role":"user"}, {"role":"assistant"}, {"role":"user"}]),
            json!([{"type":"custom_call_output", "call_id":"\n"}]),
            json!([{"type":["function_call_output"], "call_id":{"value":"x"}}]),
        ];
        for previous in [
            json!(null),
            json!(""),
            json!("  "),
            json!("resp-1"),
            json!(4),
        ] {
            for (index, input) in inputs.iter().enumerate() {
                let request = json!({"previous_response_id":previous, "input":input});
                let original = serde_json::to_string(&request).unwrap();
                let projected = runtime_request_shape_json(&request);
                assert!(projected.len() < 1024);
                for session_present in [false, true] {
                    assert_eq!(
                        runtime_proxy_request_shape(&projected, session_present),
                        runtime_proxy_request_shape(&original, session_present),
                        "input case={index}, previous={previous:?}, session={session_present}",
                    );
                }
            }
        }
        let many_items = json!({"input": vec![Value::Null; 10_000]});
        let original = serde_json::to_string(&many_items).unwrap();
        let projected = runtime_request_shape_json(&many_items);
        assert!(projected.len() < original.len());
        assert_eq!(
            runtime_proxy_request_shape(&projected, false),
            runtime_proxy_request_shape(&original, false),
        );
    }
}
