use crate::{
    assert_parsing_eq, assert_parsing_fail,
    attribute::{parse_prompt_value, AndExpression, Atom, Expression, Prompt, Term},
    Symbol,
};

#[test]
fn test_parse_prompt() {
    let input = "\"scripts/Kconfig.include\"";
    assert_parsing_eq!(
        parse_prompt_value,
        input,
        Ok(("", "scripts/Kconfig.include".to_string()))
    )
}

#[test]
fn test_parse_prompt_1() {
    let input = "\"Support in-kernel module decompression\"";
    assert_parsing_eq!(
        parse_prompt_value,
        input,
        Ok(("", "Support in-kernel module decompression".to_string()))
    )
}

// 3.0.18/arch/arm/plat-tcc/Kconfig
#[test]
fn test_parse_prompt_no_quote() {
    let input = " TCC8000";
    assert_parsing_eq!(parse_prompt_value, input, Ok(("", "TCC8000".to_string())))
}

#[test]
fn test_comment_is_not_a_prompt() {
    for input in ["# comment", " \t# comment\n", " # \"not a prompt\"\r\n"] {
        assert_parsing_fail!(parse_prompt_value, input);
    }
}

#[test]
fn test_unquoted_prompt_stops_before_comment() {
    for suffix in ["# comment", "# comment\n", "# comment\r\n"] {
        for prompt in ["TCC8000", " TCC8000 \t"] {
            let input = format!("{prompt}{suffix}");
            assert_parsing_eq!(
                parse_prompt_value,
                &input,
                Ok((suffix, "TCC8000".to_string()))
            );
        }
    }
}

#[test]
fn test_prompt_to_string() {
    assert_eq!(
        Prompt {
            prompt: "Support of KVM".to_string(),
            r#if: None
        }
        .to_string(),
        r#""Support of KVM""#
    );

    assert_eq!(
        Prompt {
            prompt: "Support of KVM".to_string(),
            r#if: Some(Expression::Term(AndExpression::Term(Term::Atom(
                Atom::Symbol(Symbol::NonConstant("KVM".to_string()))
            ))))
        }
        .to_string(),
        r#""Support of KVM" if KVM"#
    )
}

/// https://github.com/u-boot/u-boot/blob/e2fa3e570f83ab0f9ce667ddaec9dc738bcf05b9/net/Kconfig#L82-L88
#[test]
fn test_prompt_starting_with_hash() {
    for quote in ['"', '\''] {
        for prompt in ["# Support in-kernel module decompression", "Support #1"] {
            let input = format!("{quote}{prompt}{quote} # comment");
            assert_parsing_eq!(
                parse_prompt_value,
                &input,
                Ok((" # comment", prompt.to_string()))
            );
        }
    }
}
