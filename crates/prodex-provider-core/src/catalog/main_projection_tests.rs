use super::{MainCatalogModel, project_main_catalog_models};
use serde_json::json;

#[test]
fn main_catalog_projection_preserves_typed_aliases_flags_and_unsigned_observations() {
    let models = project_main_catalog_models(&[
        json!({"slug":null,"id":" \u{2003}雪 ","display_name":1,"displayName":" Label ",
            "priority":u64::MAX,"supported_in_api":"false","hidden":true,"visibility":"LiST",
            "aliases":[" a ",null,"",3,"a"],
            "supported_reasoning_levels":[{"effort":" LOW "},{"effort":null},"high",{"effort":""}],
            "default_reasoning_level":false,"default_reasoning_effort":" HIGH "}),
        json!({"slug":" ","id":"ignored","display_name":" ","displayName":"ignored",
            "priority":0,"supported_in_api":false,"hidden":"true","visibility":" list ",
            "aliases":{},"supported_reasoning_levels":{},
            "default_reasoning_level":" ","default_reasoning_effort":"high"}),
        json!(false),
    ])
    .unwrap();
    let expected: Vec<MainCatalogModel> = serde_json::from_value(json!([
        {"id":"雪","label":"Label","priority":u64::MAX,"supported":true,"hidden":true,
            "listed":true,"efforts":[" LOW ",""],"aliases":["a","a"],"default_effort":"HIGH"},
        {"id":"","label":"","priority":0,"supported":false,"hidden":false,
            "listed":false,"efforts":[],"aliases":[],"default_effort":null},
        {"id":"","label":"","priority":9223372036854775807_u64,"supported":true,"hidden":false,
            "listed":true,"efforts":[],"aliases":[],"default_effort":null},
    ]))
    .unwrap();
    assert_eq!(models, expected);
    assert_eq!(
        project_main_catalog_models(&[]).unwrap(),
        Vec::<MainCatalogModel>::new()
    );
}
