use std::{collections::HashMap, path::PathBuf};

use nom_kconfig::{
    attribute::{
        r#macro::Macro, r#type::Type, AndExpression, Atom, Expression, ExpressionToken, Term,
    },
    parse_kconfig, Attribute, Entry, KconfigFile, KconfigInput,
};

#[test]
fn standalone_variables_still_expand() {
    let file = KconfigFile::new_with_vars(
        PathBuf::new(),
        PathBuf::new(),
        &HashMap::from([("ARCH", "i386"), ("SRCARCH", "x86")]),
        &HashMap::from([("ARCH", "x86_64"), ("empty", "")]),
    );
    let input = "source \"arch/$(SRCARCH)/Kconfig\"\n\
                 default \"${ARCH}\" != \"i386\"\n\
                 # λ $(ARCH) $(missing) ${missing} $x $ $(empty)";
    assert_eq!(
        file.preprocess_content(input.to_owned()),
        "source \"arch/x86/Kconfig\"\n\
         default \"x86_64\" != \"i386\"\n\
         # λ x86_64 $(missing) ${missing} $x $ "
    );
}

#[test]
fn function_arguments_keep_variable_references() {
    let mut file = KconfigFile::default();
    file.add_local_var("comma", ",");
    file.add_local_var("ARCH", "x86_64");
    let input = "$(first,left$(comma)right) $(second,${comma},$(ARCH)) $(ARCH)";
    assert_eq!(
        file.preprocess_content(input.to_owned()),
        "$(first,left$(comma)right) $(second,${comma},$(ARCH)) x86_64"
    );
}

#[test]
fn nested_calls_and_literal_parentheses_keep_argument_boundaries() {
    let mut file = KconfigFile::default();
    file.add_local_var("comma", ",");
    let input = "$(outer,$(inner,left$(comma)right),(%rbx)$(comma)tail) $(comma)";
    assert_eq!(
        file.preprocess_content(input.to_owned()),
        "$(outer,$(inner,left$(comma)right),(%rbx)$(comma)tail) ,"
    );
}

#[test]
fn quoted_arguments_keep_variable_references() {
    let mut file = KconfigFile::default();
    file.add_local_var("comma", ",");
    let input = r#"$(shell,echo "left$(comma)right" '$(comma)') $(comma)"#;
    assert_eq!(
        file.preprocess_content(input.to_owned()),
        r#"$(shell,echo "left$(comma)right" '$(comma)') ,"#
    );
}

#[test]
fn unterminated_reference_is_left_for_the_parser() {
    let mut file = KconfigFile::default();
    file.add_local_var("comma", ",");
    let input = "$(comma) $(first,left$(comma)right";
    assert_eq!(
        file.preprocess_content(input.to_owned()),
        ", $(first,left$(comma)right"
    );
}

#[test]
fn sourced_comma_definition_does_not_split_assembler_operands() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("preprocess-fixtures");
    let file = KconfigFile::new(root, PathBuf::from("main.Kconfig"));
    let content = file.read_to_string().unwrap();
    let (remaining, parsed) = parse_kconfig(KconfigInput::new_extra(&content, file)).unwrap();
    assert!(remaining.is_empty());

    let Entry::Source(source) = &parsed.entries[1] else {
        panic!("expected the assembler Kconfig source");
    };
    let Entry::Config(config) = &source.kconfigs[0].entries[0] else {
        panic!("expected AS_WRUSS");
    };
    assert_eq!(config.symbol, "AS_WRUSS");
    let Attribute::Type(ty) = &config.attributes[0] else {
        panic!("expected def_bool");
    };
    let Type::DefBool(Expression::Term(AndExpression::Term(Term::Atom(Atom::Macro(
        Macro::FunctionCall(call),
    ))))) = &ty.r#type
    else {
        panic!("expected an assembler probe, got {ty:?}");
    };

    assert_eq!(call.name, "as-instr64");
    assert_eq!(call.parameters.len(), 1, "the instruction is one argument");
    assert!(call.parameters[0]
        .tokens
        .contains(&ExpressionToken::Variable("comma".to_owned())));
    assert!(call.parameters[0]
        .tokens
        .contains(&ExpressionToken::Literal("(%rbx)".to_owned())));
}
