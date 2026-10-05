#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::smart_context_symbols::{
    SymbolIndexPlan, SymbolLabel, SymbolRangePlan, index,
};

#[test]
fn source_symbol_ranges_cover_rust_python_and_javascript() {
    let text = "#[test]\nfn r#entry() {\n    enter();\n}\n\nasync def test_parse():\n    parse()\n\nit(\"unicode 雪\", () => {});\n";

    let plan = index(text, 16, 16 * 1024).unwrap();

    assert_eq!(
        plan,
        SymbolIndexPlan {
            complete: true,
            ranges: vec![
                SymbolRangePlan {
                    start_line: 1,
                    end_line: 4,
                    declaration_line: 2,
                    label: SymbolLabel::Test,
                    symbol: "entry".to_owned(),
                },
                SymbolRangePlan {
                    start_line: 5,
                    end_line: 8,
                    declaration_line: 6,
                    label: SymbolLabel::Test,
                    symbol: "test_parse".to_owned(),
                },
                SymbolRangePlan {
                    start_line: 8,
                    end_line: 9,
                    declaration_line: 9,
                    label: SymbolLabel::Test,
                    symbol: "unicode 雪".to_owned(),
                },
            ],
        }
    );
}

#[test]
fn source_symbol_ranges_preserve_function_type_and_fallback_names() {
    let text = "function direct_name() {\n}\nconst arrow_name = input => input;\nstruct Header {}\nimpl Header {}\n\nit();\ntest();\n";

    let plan = index(text, 16, 16 * 1024).unwrap();

    assert_eq!(
        plan,
        SymbolIndexPlan {
            complete: true,
            ranges: vec![
                SymbolRangePlan {
                    start_line: 1,
                    end_line: 2,
                    declaration_line: 1,
                    label: SymbolLabel::Function,
                    symbol: "direct_name".to_owned(),
                },
                SymbolRangePlan {
                    start_line: 3,
                    end_line: 4,
                    declaration_line: 3,
                    label: SymbolLabel::Function,
                    symbol: "arrow_name".to_owned(),
                },
                SymbolRangePlan {
                    start_line: 4,
                    end_line: 4,
                    declaration_line: 4,
                    label: SymbolLabel::Symbol,
                    symbol: "Header".to_owned(),
                },
                SymbolRangePlan {
                    start_line: 5,
                    end_line: 5,
                    declaration_line: 5,
                    label: SymbolLabel::Symbol,
                    symbol: "impl Header".to_owned(),
                },
                SymbolRangePlan {
                    start_line: 6,
                    end_line: 8,
                    declaration_line: 7,
                    label: SymbolLabel::Test,
                    symbol: "it".to_owned(),
                },
                SymbolRangePlan {
                    start_line: 8,
                    end_line: 8,
                    declaration_line: 8,
                    label: SymbolLabel::Test,
                    symbol: "test".to_owned(),
                },
            ],
        }
    );
}

#[test]
fn symbol_index_reports_capacity_and_excerpt_truncation() {
    let text = "fn first() {}\nfn second() {}\n";

    assert_eq!(
        index(text, 1, 16 * 1024).unwrap(),
        SymbolIndexPlan {
            complete: false,
            ranges: vec![SymbolRangePlan {
                start_line: 1,
                end_line: 1,
                declaration_line: 1,
                label: SymbolLabel::Function,
                symbol: "first".to_owned(),
            }],
        }
    );
    assert_eq!(
        index("fn too_large() {}", 1, 4).unwrap(),
        SymbolIndexPlan {
            complete: false,
            ranges: Vec::new(),
        }
    );
}

#[test]
fn symbol_index_accepts_large_multi_line_artifacts_with_bounded_lines() {
    let line = format!("// {}", "x".repeat(2_100_000));
    let text = format!("{line}\n{line}");
    assert!(text.len() > 4 * 1024 * 1024);

    assert_eq!(
        index(&text, 256, 16 * 1024).unwrap(),
        SymbolIndexPlan {
            complete: true,
            ranges: Vec::new(),
        }
    );
}
