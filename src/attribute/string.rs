use nom::{bytes::complete::tag, combinator::map, sequence::delimited, IResult, Parser};

pub use crate::string::take_until_unbalanced;
use crate::KconfigInput;

pub fn parse_string(input: KconfigInput) -> IResult<KconfigInput, String> {
    map(
        delimited(tag("\""), take_until_unbalanced('"'), tag("\"")),
        |d| d.fragment().to_string(),
    )
    .parse(input)
}
