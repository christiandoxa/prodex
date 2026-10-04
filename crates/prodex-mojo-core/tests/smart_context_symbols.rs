#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::smart_context_symbols::{
    SymbolLabel,
    SymbolLabel::{Function, Symbol, Test},
    SymbolStyle,
    SymbolStyle::{Brace, Python},
    visit_classifications,
};

fn classifications(text: &str) -> Vec<(usize, SymbolLabel, String, SymbolStyle)> {
    let mut found = Vec::new();
    visit_classifications(&text.lines().collect::<Vec<_>>(), |item| {
        found.push((item.line_index, item.label, item.symbol, item.style));
        true
    })
    .unwrap();
    found
}

#[test]
fn mojo_classifies_rust_python_and_javascript_declaration_names() {
    assert_eq!(
        classifications(
            "#[test]\nfn r#entry() {}\nasync def test_parse():\nfunction direct() {}\nconst arrow = x => x;\nstruct Header {}\nimpl Header {}\nit(\"unicode 雪\", () => {});\ntest();\nit();"
        ),
        vec![
            (1, Test, "entry".into(), Brace),
            (2, Test, "test_parse".into(), Python),
            (3, Function, "direct".into(), Brace),
            (4, Function, "arrow".into(), Brace),
            (5, Symbol, "Header".into(), Brace),
            (6, Symbol, "impl Header".into(), Brace),
            (7, Test, "unicode 雪".into(), Brace),
            (8, Test, "test".into(), Brace),
            (9, Test, "it".into(), Brace),
        ]
    );
}

#[test]
fn mojo_classifies_test_attributes_and_declaration_families() {
    assert_eq!(
        classifications("#[rstest]\n#[case]\nfn case_one() {}\n#[tokio::test]\nasync fn tokio_case() {}\n#[async_std::test]\nfn std_case() {}\nfn ordinary() {}\nenum Mode {}\ntrait Runner {}\nmod nested {}\nclass Widget {}")
            .iter()
            .map(|item| (item.0, item.1, item.2.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (2, Test, "case_one"),
            (4, Test, "tokio_case"),
            (6, Test, "std_case"),
            (7, Function, "ordinary"),
            (8, Symbol, "Mode"),
            (9, Symbol, "Runner"),
            (10, Symbol, "nested"),
            (11, Symbol, "Widget"),
        ]
    );
}
