use nom_kconfig::{
    attribute::{
        expression::{
            AndExpression, Atom, CompareExpression, CompareOperand, CompareOperator, Term,
        },
        parse_default, parse_expression,
        r#type::{ConfigType, Type},
        DefaultAttribute, Expression,
    },
    entry::config::Config,
    parse_kconfig,
    string::{parse_first_word, parse_string},
    symbol::ConstantSymbol,
    Attribute, Entry, KconfigFile, KconfigInput, Symbol,
};

fn input(text: &str) -> KconfigInput<'_> {
    KconfigInput::new_extra(text, KconfigFile::default())
}

fn string_expression(text: &str) -> Expression {
    Expression::Term(AndExpression::Term(Term::Atom(Atom::Symbol(
        Symbol::Constant(ConstantSymbol::String(text.to_owned())),
    ))))
}

fn comparison(left: &str, operator: CompareOperator, right: &str) -> Expression {
    Expression::Term(AndExpression::Term(Term::Atom(Atom::Compare(
        CompareExpression {
            left: CompareOperand::Symbol(Symbol::Constant(ConstantSymbol::String(left.to_owned()))),
            operator,
            right: CompareOperand::Symbol(Symbol::Constant(ConstantSymbol::String(
                right.to_owned(),
            ))),
        },
    ))))
}

#[test]
fn quoted_tokens_stop_at_the_first_unescaped_matching_quote() {
    for (source, token, value, rest) in [
        (
            r#""x86_64" != "i386""#,
            r#""x86_64""#,
            "x86_64",
            r#" != "i386""#,
        ),
        ("'x86_64' != 'i386'", "'x86_64'", "x86_64", " != 'i386'"),
        (r#""a\"b" = "c""#, r#""a\"b""#, r#"a\"b"#, r#" = "c""#),
        (r#"'a\'b' = 'c'"#, r#"'a\'b'"#, r#"a\'b"#, " = 'c'"),
        (r#""path\\" != """#, r#""path\\""#, r#"path\\"#, r#" != """#),
        (r#""a\\\"b" = "c""#, r#""a\\\"b""#, r#"a\\\"b"#, r#" = "c""#),
        (r#""αβ" != "γ""#, r#""αβ""#, "αβ", r#" != "γ""#),
        (
            r#""say 'hello'" = "other""#,
            r#""say 'hello'""#,
            "say 'hello'",
            r#" = "other""#,
        ),
        (r#""a" "b""#, r#""a""#, "a", r#" "b""#),
        (r#""""#, r#""""#, "", ""),
    ] {
        let (remaining, parsed) = parse_string(input(source)).unwrap();
        assert_eq!(
            (*remaining.fragment(), parsed.as_str()),
            (rest, value),
            "{source}"
        );

        let (remaining, parsed) = parse_first_word(input(source)).unwrap();
        assert_eq!(
            (*remaining.fragment(), *parsed.fragment()),
            (rest, token),
            "{source}"
        );

        if source.starts_with('"') {
            let (remaining, parsed) =
                nom_kconfig::attribute::string::parse_string(input(source)).unwrap();
            assert_eq!(
                (*remaining.fragment(), parsed.as_str()),
                (rest, value),
                "{source}"
            );
        }
    }
}

#[test]
fn quoted_tokens_require_a_closing_quote_on_the_same_line() {
    for source in [
        "\"",
        "'",
        "\"unterminated",
        "'unterminated",
        r#""escaped\""#,
        r#"'escaped\'"#,
        "\"first\nsecond\"",
        "\"first\r\nsecond\"",
        "\"first\\\nsecond\"",
    ] {
        assert!(parse_string(input(source)).is_err(), "{source:?}");
        if source.starts_with('"') {
            assert!(
                nom_kconfig::attribute::string::parse_string(input(source)).is_err(),
                "{source:?}"
            );
        }
    }
}

#[test]
fn quoted_operands_remain_comparisons() {
    for (operator, expected) in [
        ("=", CompareOperator::Equal),
        ("!=", CompareOperator::NotEqual),
        ("<", CompareOperator::LowerThan),
        ("<=", CompareOperator::LowerOrEqual),
        (">", CompareOperator::GreaterThan),
        (">=", CompareOperator::GreaterOrEqual),
    ] {
        for (left_quote, right_quote) in [('"', '"'), ('\'', '\''), ('"', '\''), ('\'', '"')] {
            for space in ["", " "] {
                let source = format!("{left_quote}x86_64{left_quote}{space}{operator}{space}{right_quote}i386{right_quote}");
                let (remaining, parsed) = parse_expression(input(&source)).unwrap();
                assert!(remaining.is_empty(), "{source}");
                assert_eq!(
                    parsed,
                    comparison("x86_64", expected.clone(), "i386"),
                    "{source}"
                );
            }
        }
    }
}

#[test]
fn quoted_default_keeps_its_condition() {
    let source = r#"default "chosen" if "x86_64" != "i386""#;
    let (remaining, parsed) = parse_default(input(source)).unwrap();
    assert!(remaining.is_empty());
    assert_eq!(
        parsed,
        DefaultAttribute {
            expression: string_expression("chosen"),
            r#if: Some(comparison("x86_64", CompareOperator::NotEqual, "i386")),
        }
    );
}

#[test]
fn preprocessed_x86_64bit_prompt_and_default_are_comparisons() {
    let source = r#"config 64BIT
    bool "64-bit kernel" if "$(ARCH)" = "x86"
    default "$(ARCH)" != "i386"
"#;
    for arch in ["x86", "x86_64", "i386"] {
        let mut file = KconfigFile::default();
        file.add_local_var("ARCH", arch);
        let expanded = file.preprocess_content(source.to_owned());
        let (remaining, parsed) = parse_kconfig(KconfigInput::new_extra(&expanded, file)).unwrap();
        assert!(remaining.is_empty());
        assert_eq!(
            parsed.entries,
            vec![Entry::Config(Config {
                symbol: "64BIT".to_owned(),
                attributes: vec![
                    Attribute::Type(ConfigType {
                        r#type: Type::Bool(Some("64-bit kernel".to_owned())),
                        r#if: Some(comparison(arch, CompareOperator::Equal, "x86")),
                    }),
                    Attribute::Default(DefaultAttribute {
                        expression: comparison(arch, CompareOperator::NotEqual, "i386"),
                        r#if: None,
                    }),
                ],
            })],
            "ARCH={arch}"
        );
    }
}
