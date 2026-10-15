use crate::{
    assert_parsing_eq,
    attribute::r#type::{ConfigType, Type},
    entry::config::Config,
    kconfig::parse_kconfig,
    Attribute, Entry, Kconfig, KconfigInput,
};

#[cfg(feature = "glob-wildcard")]
use crate::attribute::expression::CompareOperand;
#[cfg(feature = "glob-wildcard")]
use crate::attribute::{r#macro::Macro, DefaultAttribute};
#[cfg(feature = "glob-wildcard")]
use crate::attribute::{AndExpression, Atom, CompareExpression, CompareOperator, Expression, Term};
#[cfg(not(feature = "named-choice"))]
use crate::entry::Choice;
#[cfg(feature = "glob-wildcard")]
use crate::symbol::ConstantSymbol;
#[cfg(feature = "glob-wildcard")]
use crate::Symbol;

#[test]
fn test_parse_kconfig() {
    let input = "
    config SND_INTEL_NHLT
        tristate
        # this config should be selected only for Intel ACPI platforms.
        # A fallback is provided so that the code compiles in all cases.";
    assert_parsing_eq!(
        parse_kconfig,
        input,
        Ok((
            "",
            Kconfig {
                file: "".to_string(),
                entries: vec!(Entry::Config(Config {
                    symbol: "SND_INTEL_NHLT".to_string(),
                    attributes: vec!(Attribute::Type(ConfigType {
                        r#type: Type::Tristate(None),
                        r#if: None
                    }))
                }))
            }
        ))
    )
}

#[test]
fn test_inline_type_comment_does_not_add_prompt() {
    for (keyword, expected_type) in [
        ("bool", Type::Bool(None)),
        ("boolean", Type::Bool(None)),
        ("tristate", Type::Tristate(None)),
        ("int", Type::Int(None)),
        ("hex", Type::Hex(None)),
        ("string", Type::String(None)),
    ] {
        for suffix in ["# comment", " \t# comment\n", " # comment\r\n"] {
            let input = format!("config X\n\t{keyword}{suffix}");
            assert_parsing_eq!(
                parse_kconfig,
                &input,
                Ok((
                    "",
                    Kconfig {
                        file: "".to_string(),
                        entries: vec![Entry::Config(Config {
                            symbol: "X".to_string(),
                            attributes: vec![Attribute::Type(ConfigType {
                                r#type: expected_type.clone(),
                                r#if: None,
                            })],
                        })],
                    }
                ))
            );
        }
    }
}

#[test]
fn test_rt5514_inline_comment_preserves_hidden_default() {
    let comment = " # force RT5514_SPI to be built-in to avoid link errors";
    let input = format!(
        "config SND_SOC_RT5514_SPI_BUILTIN\n\tbool{comment}\n\
         \tdefault SND_SOC_RT5514=y && SND_SOC_RT5514_SPI=m\n"
    );
    let without_comment = input.replace(comment, "");
    let (_, actual) = parse_kconfig(KconfigInput::new_extra(&input, Default::default())).unwrap();
    let (_, expected) = parse_kconfig(KconfigInput::new_extra(
        &without_comment,
        Default::default(),
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
#[cfg(not(feature = "named-choice"))]
fn test_parse_kconfig_choice() {
    let input = "
choice
config RAPIDIO_ENUM_BASIC
	tristate

endchoice
";
    assert_parsing_eq!(
        parse_kconfig,
        input,
        Ok((
            "",
            Kconfig {
                file: "".to_string(),
                entries: vec!(Entry::Choice(Choice {
                    options: vec!(),
                    entries: vec!(Entry::Config(Config {
                        symbol: "RAPIDIO_ENUM_BASIC".to_string(),
                        attributes: vec!(Attribute::Type(ConfigType {
                            r#type: Type::Tristate(None),
                            r#if: None
                        }))
                    }))
                })),
            }
        ))
    )
}

#[test]
#[cfg(feature = "glob-wildcard")]
/// https://github.com/Mcdostone/nom-kconfig/issues/107#issuecomment-3736187967
fn test_parse_kconfig_bool() {
    let input = r#"
config 64BIT
	bool "64-bit kernel" if "$(SUBARCH)" = "x86"
	default "$(SUBARCH)" != "i386"
"#;
    assert_parsing_eq!(
        parse_kconfig,
        input,
        Ok((
            "",
            Kconfig {
                file: "".to_string(),
                entries: vec!(Entry::Config(Config {
                    symbol: "64BIT".to_string(),
                    attributes: vec!(
                        Attribute::Type(ConfigType {
                            r#type: Type::Bool(Some("64-bit kernel".to_string())),
                            r#if: Some(Expression::Term(AndExpression::Term(Term::Atom(
                                Atom::Compare(CompareExpression {
                                    left: CompareOperand::Macro(Macro::DoubleQuoted(Box::new(
                                        Macro::Variable("SUBARCH".to_string())
                                    ))),
                                    operator: CompareOperator::Equal,
                                    right: CompareOperand::Symbol(Symbol::Constant(
                                        ConstantSymbol::String("x86".to_string())
                                    ))
                                })
                            )))),
                        }),
                        Attribute::Default(DefaultAttribute {
                            expression: Expression::Term(AndExpression::Term(Term::Atom(
                                Atom::Compare(CompareExpression {
                                    left: CompareOperand::Macro(Macro::DoubleQuoted(Box::new(
                                        Macro::Variable("SUBARCH".to_string())
                                    ))),
                                    operator: CompareOperator::NotEqual,
                                    right: CompareOperand::Symbol(Symbol::Constant(
                                        ConstantSymbol::String("i386".to_string())
                                    ))
                                })
                            ))),
                            r#if: None
                        })
                    ),
                }))
            }
        ))
    )
}

#[test]
#[cfg(feature = "kconfiglib")]
#[ignore]
/// https://github.com/zephyrproject-rtos/zephyr/blob/main/boards/Kconfig.v2
fn test_parse_kconfig_with_external_functions() {
    use crate::{
        attribute::{ExpressionToken, FunctionCall, Parameter},
        entry::{Value, VariableAssignment, VariableIdentifier},
    };

    let input = "
    BOARD_STRING := hello
BOARD_TARGET_STRING := world

config BOARD_$(BOARD_STRING)
	def_bool y
	help
	  Kconfig symbol identifying the board.
#";
    assert_parsing_eq!(
        parse_kconfig,
        input,
        Ok((
            "",
            Kconfig {
                file: "".to_string(),
                entries: vec!(
                    Entry::VariableAssignment(VariableAssignment {
                        identifier: VariableIdentifier::Identifier("BOARD_STRING".to_string()),
                        operator: ":=".to_string(),
                        right: Value::FunctionCall(FunctionCall {
                            name: "normalize_upper".to_string(),
                            parameters: vec![Parameter {
                                tokens: vec![ExpressionToken::Variable("BOARD".to_string())],
                            }],
                        })
                    }),
                    Entry::VariableAssignment(VariableAssignment {
                        identifier: VariableIdentifier::Identifier(
                            "BOARD_TARGET_STRING".to_string()
                        ),
                        operator: ":=".to_string(),
                        right: Value::FunctionCall(FunctionCall {
                            name: "normalize_upper".to_string(),
                            parameters: vec![Parameter {
                                tokens: vec![
                                    ExpressionToken::Variable("BOARD".to_string()),
                                    ExpressionToken::Literal("/".to_string()),
                                    ExpressionToken::Variable("BOARD_QUALIFIERS".to_string())
                                ],
                            }],
                        })
                    }),
                    Entry::Config(Config {
                        symbol: "BOARD_$(BOARD_STRING)".to_string(),
                        attributes: vec!(Attribute::Type(ConfigType {
                            r#type: Type::Bool(Some("y".to_string())),
                            r#if: None
                        }))
                    })
                )
            }
        ))
    )
}
