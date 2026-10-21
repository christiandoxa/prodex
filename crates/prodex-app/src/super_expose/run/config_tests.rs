use super::apply_overrides;
use prodex_cli::{Commands, SuperArgs};
use serde_json::json;

fn args() -> SuperArgs {
    let Commands::Super(args) =
        prodex_cli::parse_cli_command_from(["prodex", "s", "--no-presidio"]).unwrap()
    else {
        panic!("expected Super args");
    };
    *args
}

#[test]
fn compiled_override_text_validation_preserves_null_types_and_failures() {
    let mut args = args();
    args.local_model = Some("retained".into());
    apply_overrides(&mut args, &json!({"model":null})).unwrap();
    assert_eq!(args.local_model.as_deref(), Some("retained"));
    assert_eq!(
        apply_overrides(&mut args, &json!({"model":false})),
        Err("model must be a string".into())
    );
    for value in [
        String::new(),
        "\0".into(),
        "line\nnext".into(),
        "\u{0085}".into(),
        "é".repeat(129),
    ] {
        assert_eq!(
            apply_overrides(&mut args, &json!({"model":value})),
            Err("model is empty or too large".into())
        );
        assert_eq!(args.local_model.as_deref(), Some("retained"));
    }
}

#[test]
fn compiled_override_text_validation_keeps_unicode_bytes_and_no_trimming() {
    let mut args = args();
    for value in [" é雪🙂 ".to_owned(), "\u{200B}".to_owned(), "é".repeat(128)] {
        apply_overrides(&mut args, &json!({"model":value})).unwrap();
        assert_eq!(args.local_model.as_deref(), Some(value.as_str()));
    }
}
