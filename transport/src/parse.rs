use nom::{
    bytes::{
        complete::{tag, take_until},
        streaming,
    },
    combinator::{map, map_res, rest},
    error::{context, VerboseError},
    multi::many1,
};

use crate::Message;

type Res<I, O> = nom::IResult<I, O, VerboseError<I>>;

struct Headers {
    content_length: usize,
}

struct KeyValue {
    key: String,
    value: usize,
}

fn parse_header(input: &str) -> Res<&str, KeyValue> {
    let (input, key) = context("reading key", map(take_until(":"), |s: &str| s.trim()))(input)?;
    let (input, _) = context("colon separator", tag(":"))(input)?;
    let (input, value) =
        context("reading value", map_res(rest, |s: &str| s.trim().parse()))(input)?;

    Ok((
        input,
        KeyValue {
            key: key.to_owned(),
            value,
        },
    ))
}

fn parse_header_str(input: &str) -> Res<&str, Headers> {
    let (input, key_values) = context("parsing headers", many1(parse_header))(input)?;
    for kv in key_values {
        if kv.key == "Content-Length" {
            return Ok((
                input,
                Headers {
                    content_length: kv.value,
                },
            ));
        }
    }

    todo!()
}

fn parse_headers(input: &str) -> Res<&str, Headers> {
    let (input, header_str) = context("parsing headers", take_until("\r\n\r\n"))(input)?;
    let (_, headers) = parse_header_str(header_str)?;
    let (input, _) = tag("\r\n\r\n")(input)?;
    Ok((input, headers))
}

pub(crate) fn parse_message(input: &str) -> Res<&str, Message> {
    let (input, Headers { content_length }) = parse_headers(input)?;
    let (input, message) = context(
        "reading JSON body",
        map_res(streaming::take(content_length), |s| serde_json::from_str(s)),
    )(input)?;
    Ok((input, message))
}

#[cfg(test)]
mod tests {
    use crate::events;

    use super::*;

    #[test]
    fn parse_request() {
        let input = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}";
        match parse_message(input) {
            Ok((rest, parsed)) => {
                assert_eq!(rest, "");
                assert!(matches!(parsed, Message::Event(events::Event::Terminated)));
            }
            Err(nom::Err::Incomplete(why)) => panic!("incomplete data, why: {why:?}"),
            Err(e) => panic!("error parsing headers: {e}"),
        }
    }

    #[test]
    fn parse_request_with_trailing() {
        let input = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"terminated\"}Conten";
        match parse_message(input) {
            Ok((rest, parsed)) => {
                assert_eq!(rest, "Conten");
                assert!(matches!(parsed, Message::Event(events::Event::Terminated)));
            }
            Err(nom::Err::Incomplete(why)) => panic!("incomplete data, why: {why:?}"),
            Err(e) => panic!("error parsing headers: {e}"),
        }
    }

    #[test]
    fn parse_incomplete() {
        let input = "Content-Length: 37\r\n\r\n{\"type\":\"event\",\"event\":\"termi";
        assert!(matches!(parse_message(input), Err(nom::Err::Incomplete(_))));
    }
}
