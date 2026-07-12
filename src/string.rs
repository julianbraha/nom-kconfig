use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alphanumeric1, one_of},
    combinator::{map, recognize},
    error::{Error, ErrorKind, ParseError},
    multi::many1,
    sequence::delimited,
    IResult, Input, Parser,
};

use crate::{util::ws, KconfigInput};

pub fn parse_string(input: KconfigInput) -> IResult<KconfigInput, String> {
    map(
        alt((
            delimited(tag("'"), take_until_unbalanced('\''), tag("'")),
            delimited(tag("\""), take_until_unbalanced('"'), tag("\"")),
        )),
        |d| d.fragment().to_string(),
    )
    .parse(input)
}

/// Takes the content of a string whose opening `delimiter` has already been consumed.
/// The string must close before the end of the line.
///
/// Kconfig files in the wild contain strings with unescaped nested quotes
/// (e.g. `"hello "world"" if NET`), so the closing quote is not simply the next
/// delimiter: it is the first delimiter that is followed by a character allowed
/// after a string (whitespace, an operator, ...) and that leaves an even number of
/// delimiters behind it on the line, so they can still pair up. When no delimiter
/// qualifies, the string extends to the last delimiter on the line.
pub fn take_until_unbalanced(
    delimiter: char,
) -> impl Fn(KconfigInput) -> IResult<KconfigInput, KconfigInput> {
    move |i: KconfigInput| {
        let end_of_line = match i.find('\n') {
            Some(e) => e,
            None => i.len(),
        };
        let line: &str = &i[..end_of_line];

        // positions of the delimiters on the line, ignoring backslash-escaped ones
        let mut positions = Vec::new();
        let mut escaped = false;
        for (index, c) in line.char_indices() {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == delimiter {
                positions.push(index);
            }
        }

        // together with the already consumed opening delimiter, the delimiters on
        // the line must pair up, otherwise a quote is left unbalanced
        if positions.len() % 2 == 0 {
            return Err(nom::Err::Error(Error::from_error_kind(
                i,
                ErrorKind::TakeUntil,
            )));
        }

        let closes_string = |position: usize| match line[position..].chars().nth(1) {
            None => true,
            Some(c) => c.is_whitespace() || "=!<>&|),#".contains(c),
        };
        let index = positions
            .iter()
            .step_by(2)
            .copied()
            .find(|position| closes_string(*position))
            .unwrap_or_else(|| *positions.last().unwrap());

        Ok(i.take_split(index))
    }
}

/// A first word is `'something here'` or `"something here"` or just a normal word without spaces. It is used in places where Kconfig allows either a string or a symbol, such as in `default` attributes.
pub fn parse_first_word(input: KconfigInput) -> IResult<KconfigInput, KconfigInput> {
    alt((
        recognize((tag("'"), take_until_unbalanced('\''), tag("'"))),
        recognize((tag("\""), take_until_unbalanced('"'), tag("\""))),
        recognize(ws(many1(alt((alphanumeric1, recognize(one_of("-._'\""))))))),
    ))
    .parse(input)
}
