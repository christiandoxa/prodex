//! Link and execute the runtime JSON protocol with only `mojo-runtime` enabled.
#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::json::{JsonKind, JsonNode};
use prodex_mojo_core::runtime::thread_index::{
    ThreadIndexProtocol, ThreadIndexProtocolStep, ThreadIndexScope,
};

fn empty_result_nodes() -> [JsonNode<'static>; 3] {
    [
        JsonNode {
            kind: JsonKind::Object,
            first_child: Some(1),
            next_sibling: None,
            parent: None,
            key: "",
            text: "",
            raw_start: 0,
            raw_length: 20,
        },
        JsonNode {
            kind: JsonKind::Number,
            first_child: None,
            next_sibling: Some(2),
            parent: Some(0),
            key: "id",
            text: "",
            raw_start: 6,
            raw_length: 1,
        },
        JsonNode {
            kind: JsonKind::Object,
            first_child: None,
            next_sibling: None,
            parent: Some(0),
            key: "result",
            text: "",
            raw_start: 17,
            raw_length: 2,
        },
    ]
}

#[test]
fn runtime_only_json_links_and_drives_thread_index_protocol() {
    let (mut protocol, start) =
        ThreadIndexProtocol::start(ThreadIndexScope::Latest, "runtime-only-regression").unwrap();
    let ThreadIndexProtocolStep::Send(messages) = start else {
        panic!("Mojo must initialize the protocol before listing threads");
    };
    assert_eq!(messages.len(), 1);
    assert!(
        std::str::from_utf8(&messages[0])
            .unwrap()
            .contains("\"method\":\"initialize\"")
    );

    let nodes = empty_result_nodes();
    let initialized = br#"{"id":1,"result":{}}"#;
    let step = protocol
        .response(&nodes, initialized, initialized.len())
        .unwrap();
    let ThreadIndexProtocolStep::Send(messages) = step else {
        panic!("Mojo must acknowledge initialization and request the newest page");
    };
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], br#"{"method":"initialized"}"#);
    let list = std::str::from_utf8(&messages[1]).unwrap();
    assert!(list.contains("\"method\":\"thread/list\""));
    assert!(list.contains("\"limit\":1"));
    assert!(list.contains("\"archived\":false"));

    let page = br#"{"id":2,"result":{}}"#;
    assert_eq!(
        protocol.response(&nodes, page, page.len()).unwrap(),
        ThreadIndexProtocolStep::Done,
    );
}

#[test]
fn runtime_only_json_preserves_protocol_error_mapping() {
    let (protocol, _) = ThreadIndexProtocol::start(ThreadIndexScope::Latest, "test").unwrap();
    let ThreadIndexProtocolStep::Error(message) = protocol.invalid_json(3).unwrap() else {
        panic!("invalid JSON must be rejected by the Mojo protocol owner");
    };
    assert!(message.contains("invalid JSON"));
    let ThreadIndexProtocolStep::Error(message) = protocol.eof().unwrap() else {
        panic!("EOF must not be mistaken for successful reconciliation");
    };
    assert!(message.contains("stopped"));
}
