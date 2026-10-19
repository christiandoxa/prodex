use super::*;
use serde_json::{Value, json};

#[test]
fn compiled_gemini_descriptions_preserve_alias_expansion_and_namespace_suffix_rules() {
    for (names, marker) in [
        (
            &[
                "read_file",
                "read",
                " Name.Read-File ",
                "mcp__repo__read_file",
            ][..],
            "Read a file with targeted, surgical ranges.",
        ),
        (
            &["read_many_files", "glob"],
            "Read multiple files by paths or globs;",
        ),
        (&["grep", "rg", "rip_grep", "search"], "Search text fast"),
        (
            &["exec_command", "run_shell_command", "shell", "bash"],
            "Run a shell command.",
        ),
        (&["write_file", "write"], "Write complete file content."),
        (
            &["apply_patch", "replace", "edit"],
            "Apply targeted file edits",
        ),
    ] {
        let expected = gemini_provider_core_gemini3_tool_description(names[0]).unwrap();
        assert!(expected.starts_with(marker));
        for name in names {
            assert_eq!(
                gemini_provider_core_gemini3_tool_description(name),
                Some(expected)
            );
        }
    }
    for name in [
        "mcp__repo__read",
        "mcp__repo__glob",
        "mcp__repo__search",
        "mcp__repo__bash",
        "mcp__repo__write",
        "unknown",
        "",
        "réad_file",
        "read_file\0",
    ] {
        assert_eq!(
            gemini_provider_core_gemini3_tool_description(name),
            None,
            "{name:?}"
        );
    }
    assert!(
        gemini_provider_core_gemini3_tool_description("apply_patch")
            .unwrap()
            .contains("*** Begin Patch\\n*** Add File: note.txt\\n+hello\\n*** End Patch.")
    );
}

#[test]
fn compiled_gemini_overrides_preserve_existing_parameters_and_fill_only_selected_objects() {
    let source = vec![
        json!({"name":"read_file", "description":"old", "parameters":{"properties":{
            "path":{"type":"string"}, "start_line":{"type":"integer","description":null},
            "end_line":{"type":"integer"}, "other":false
        }}}),
        json!({"name":"grep", "parameters":{"properties":{"pattern":{"description":7}}}}),
        json!({"name":"grep", "parameters":{"properties":{"pattern":null}}}),
        json!({"name":"read_file", "parameters":null}),
        json!({"name":"unrecognized", "description":"kept", "parameters":{"extra":[0,-0.0]}}),
        json!({"name":7, "description":"kept"}),
        json!(null),
    ];
    let mut actual = source.clone();
    gemini_provider_core_apply_gemini3_tool_declaration_overrides("gemini-2.5-pro", &mut actual);
    assert_eq!(actual, source);
    gemini_provider_core_apply_gemini3_tool_declaration_overrides("AUTO", &mut actual);
    let mut expected = source;
    for declaration in expected.iter_mut().take(4) {
        let name = declaration["name"].as_str().unwrap();
        declaration["description"] =
            json!(gemini_provider_core_gemini3_tool_description(name).unwrap());
    }
    expected[0]["parameters"]["properties"]["end_line"]["description"] =
        json!("1-based last line to read, inclusive.");
    assert_eq!(actual, expected);
}

#[test]
fn compiled_gemini_patch_overrides_create_null_paths_and_replace_only_input_description() {
    let source = vec![
        json!({"name":"apply_patch"}),
        json!({"name":"replace","parameters":null}),
        json!({"name":"edit","parameters":{"properties":null,"required":["input"]}}),
        json!({"name":"mcp__repo__apply_patch","parameters":{"properties":{
            "input":{"description":"old","type":"string"},"other":{"description":"kept"}
        }}}),
    ];
    let mut actual = source.clone();
    gemini_provider_core_apply_gemini3_tool_declaration_overrides("gemini-3-pro", &mut actual);
    let mut expected = source;
    for declaration in &mut expected {
        declaration["description"] =
            json!(gemini_provider_core_gemini3_tool_description("apply_patch").unwrap());
        declaration["parameters"]["properties"]["input"]["description"] = json!(
            "Full apply_patch grammar string. For Add File, prefix every new file content line with '+'."
        );
    }
    assert_eq!(actual, expected);
    for parameters in [
        json!(7),
        json!({"properties":false}),
        json!({"properties":{"input":[]}}),
    ] {
        let mut declarations = vec![json!({"name":"apply_patch","parameters":parameters})];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            gemini_provider_core_apply_gemini3_tool_declaration_overrides(
                "auto",
                &mut declarations,
            );
        }));
        assert!(result.is_err());
    }
}

#[test]
fn compiled_gemini_description_overrides_reach_the_production_declaration_builder() {
    let request = json!({"tools":[
        {"type":"function","function":{"name":"read_file","parameters":{"type":"object","properties":{"start_line":{"type":"integer"}}}}},
        {"type":"function","function":{"name":"apply_patch","parameters":{"type":"object"}}}
    ]});
    let tools =
        crate::gemini_provider_core_function_tools_from_chat(&request, "gemini-3-pro", |_| {})
            .unwrap()
            .unwrap();
    assert_eq!(
        tools[0]["functionDeclarations"][0]["parameters"]["properties"]["start_line"]["description"],
        json!("1-based first line to read when a targeted range is enough.")
    );
    assert_eq!(
        tools[0]["functionDeclarations"][1]["parameters"]["properties"]["input"]["description"],
        json!(
            "Full apply_patch grammar string. For Add File, prefix every new file content line with '+'."
        )
    );

    let mut batch = (0..1024)
        .map(|index| json!({"name":"read_file","ordinal":index}))
        .collect::<Vec<Value>>();
    gemini_provider_core_apply_gemini3_tool_declaration_overrides("auto", &mut batch);
    assert_eq!(batch, (0..1024).map(|index| json!({"name":"read_file","ordinal":index,"description":gemini_provider_core_gemini3_tool_description("read_file").unwrap()})).collect::<Vec<Value>>());
}
