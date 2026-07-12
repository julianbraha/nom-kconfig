use nom::combinator::all_consuming;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alphanumeric1, one_of},
    combinator::{map, map_parser, recognize},
    multi::many1,
    IResult, Parser,
};
#[cfg(feature = "deserialize")]
use serde::Deserialize;
#[cfg(feature = "serialize")]
use serde::Serialize;

use crate::attribute::parse_function_call;
use crate::{
    attribute::{
        function::{parse_expression_token_variable_parameter, ExpressionToken},
        FunctionCall,
    },
    string::parse_string,
    util::{parse_until_eol, ws},
    KconfigInput,
};

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "hash", derive(Hash))]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
pub struct VariableAssignment {
    pub identifier: VariableIdentifier,
    pub operator: String,
    pub right: Value,
}

impl VariableIdentifier {
    fn raw(&self) -> String {
        match self {
            VariableIdentifier::Identifier(s) => s.clone(),
            VariableIdentifier::VariableRef(reff) => reff
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "hash", derive(Hash))]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
pub enum VariableIdentifier {
    Identifier(String),
    VariableRef(Vec<ExpressionToken>),
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "hash", derive(Hash))]
#[cfg_attr(feature = "serialize", derive(Serialize))]
#[cfg_attr(feature = "deserialize", derive(Deserialize))]
pub enum Value {
    Literal(String),
    ExpandedVariable(String),
    FunctionCall(FunctionCall),
}

impl Value {
    fn raw(&self) -> String {
        match self {
            Value::Literal(s) => s.clone(),
            Value::ExpandedVariable(s) => s.clone(),
            Value::FunctionCall(function_call) => function_call.to_string(),
        }
    }
}

pub fn parse_value(input: KconfigInput) -> IResult<KconfigInput, Value> {
    alt((
        map(map_parser(parse_until_eol, parse_string), |s| {
            Value::Literal(s)
        }),
        map(
            all_consuming(map_parser(parse_until_eol, ws(parse_function_call))),
            Value::FunctionCall,
        ),
        map(parse_until_eol, |s| {
            Value::Literal(s.fragment().to_string())
        }),
    ))
    .parse(input)
}

pub fn parse_variable_identifier(input: KconfigInput) -> IResult<KconfigInput, VariableIdentifier> {
    alt((
        map(
            recognize(ws(many1(alt((alphanumeric1, recognize(one_of("-_"))))))),
            |l: KconfigInput| VariableIdentifier::Identifier(l.trim().to_string()),
        ),
        map(many1(parse_expression_token_variable_parameter), |v| {
            VariableIdentifier::VariableRef(v)
        }),
    ))
    .parse(input)
}

pub fn parse_variable_assignment(input: KconfigInput) -> IResult<KconfigInput, VariableAssignment> {
    let (mut remaining, assignment) = map(
        (
            ws(parse_variable_identifier),
            ws(parse_assign),
            ws(parse_value),
        ),
        |(l, o, r)| VariableAssignment {
            identifier: l,
            operator: o.to_string(),
            right: r,
        },
    )
    .parse(input)?;

    // If the parsing is successful, we add the variable assignment to the local variables of the KconfigFile.
    // variables can be used by the preprocessor.
    //
    // Only substitution-safe values participate: preprocess_content replaces
    // `$(NAME)` textually before parsing, so a value containing delimiters,
    // quotes, `$`, or whitespace-control characters — or an empty value —
    // could change how the surrounding text parses. The kernel's
    // scripts/Kconfig.include punctuation variables are the canonical hazard:
    // substituting `comma := ,` into `$(as-instr64,wrussq %rax$(comma)(%rbx))`
    // splits the argument, while real kconfig expands arguments only *after*
    // splitting them. Unsafe values are left to a downstream evaluator (the
    // reference stays a parsed Macro).
    let value = assignment.right.raw();
    let substitution_safe = !value.is_empty()
        && !value
            .chars()
            .any(|c| ",()\"'$".contains(c) || c.is_whitespace() || c.is_control());
    if substitution_safe {
        remaining
            .extra
            .add_local_var(assignment.identifier.raw(), value);
    }
    Ok((remaining, assignment))
}

pub fn parse_assign(input: KconfigInput<'_>) -> IResult<KconfigInput<'_>, &str> {
    map(alt((tag("="), tag(":="), tag("+="))), |d: KconfigInput| {
        d.fragment().to_owned()
    })
    .parse(input)
}

#[test]
#[ignore]
fn test_parse_value() {
    assert_eq!(
        parse_value(KconfigInput::new_extra("hello world", Default::default())),
        Ok((
            KconfigInput::new_extra("", Default::default()),
            Value::Literal("hello world".to_string())
        ))
    );

    assert_eq!(
        parse_value(KconfigInput::new_extra(
            r#""hello world""#,
            Default::default()
        )),
        Ok((
            KconfigInput::new_extra("", Default::default()),
            Value::Literal("hello world".to_string())
        ))
    );
}
