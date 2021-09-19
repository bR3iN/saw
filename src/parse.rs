use crate::program::{self, Atom, Program};
use crate::Result;

use nom::{
    self,
    branch::alt,
    character::complete::{char, digit1},
    combinator::{all_consuming, cut, map_res, opt, success, verify},
    error::{ErrorKind, FromExternalError},
    multi::{many0, separated_list0},
    sequence::{delimited, preceded, separated_pair},
    Finish, Parser,
};
use regex::Regex;
use std::fmt::{Debug, Display};

type IResult<I, O> = nom::IResult<I, O, ParseError>;

#[derive(Debug)]
enum ParseError {
    Message(anyhow::Error), // Error with a user directed message
    Internal,               // An internal error, e.g. the failure of a parser
}
impl ParseError {
    pub fn new() -> Self {
        ParseError::Internal
    }
    pub fn msg<C>(message: C) -> Self
    where
        C: Display + Debug + Send + Sync + 'static,
    {
        ParseError::Message(anyhow::Error::msg(message))
    }
}

impl<I> nom::error::ParseError<I> for ParseError {
    fn from_error_kind(_: I, _: ErrorKind) -> Self {
        ParseError::Internal
    }

    fn append(_: I, _: ErrorKind, other: Self) -> Self {
        other
    }
}

// allows the usage of map_res
impl<I, E> FromExternalError<I, E> for ParseError {
    fn from_external_error(_: I, _: ErrorKind, _: E) -> Self {
        ParseError::Internal
    }
}

impl From<ParseError> for anyhow::Error {
    fn from(source: ParseError) -> Self {
        match source {
            ParseError::Message(err) => err,
            ParseError::Internal => anyhow::Error::msg("Internal error"),
        }
    }
}

pub trait Context {
    fn context<C>(self, message: C) -> Self
    where
        C: Display + Send + Sync + 'static;
}

impl Context for ParseError {
    fn context<C>(self, message: C) -> Self
    where
        C: Display + Send + Sync + 'static,
    {
        use ParseError::*;
        match self {
            Message(err) => Message(err.context(message)),
            Internal => Message(anyhow::Error::msg(format!("{}", message))),
        }
    }
}

impl<I, O> Context for IResult<I, O> {
    fn context<C>(self, message: C) -> Self
    where
        C: Display + Send + Sync + 'static,
    {
        self.map_err(|outer| outer.map(|inner| inner.context(message)))
    }
}

// Adds a context to the parser that might depend on the input
fn add_context<I, F, O, C>(
    mut message_from: impl FnMut(I) -> C,
    mut parser: F,
) -> impl FnMut(I) -> IResult<I, O>
where
    C: Display + Send + Sync + 'static,
    F: Parser<I, O, ParseError>,
    I: Clone,
{
    move |input: I| {
        let msg = message_from(input.clone()); // clone here before move
        parser.parse(input).context(msg)
    }
}

type Args<'i> = &'i [&'i str];

pub fn parse_args(input: Args) -> Result<Program> {
    let enumeration = command(
        &["enumerate", "enum", "e", "#"],
        success(())
            .map(|_| program::Enumeration::new())
            .map(From::from),
    );

    let filter = command(
        &["filter", "f"],
        next.and_then(arg(regex))
            .map(program::Filter::new)
            .map(From::from),
    );

    let match_ = command(
        &["match", "m"],
        next.and_then(arg(regex))
            .map(program::Match::new)
            .map(From::from),
    );

    let sub = command(
        &["sub", "s"],
        next.and_then(arg(regex))
            .and(next.map(|s: &str| s.to_string()))
            .map(|(regex, s)| program::Sub::new(regex, s))
            .map(From::from),
    );

    let gsub = command(
        &["gsub", "gs"],
        next.and_then(arg(regex))
            .and(next.map(From::from))
            .map(|(regex, s)| program::Gsub::new(regex, s))
            .map(From::from),
    );

    let match_range = command(
        &["match-range", "mr"],
        next.and_then(arg(regex))
            .and(next.and_then(arg(regex)))
            .map(|(start, end)| program::MatchRange::new(start, end))
            .map(From::from),
    );

    let filter_range = command(
        &["filter-range", "fr"],
        next.and_then(arg(regex))
            .and(next.and_then(arg(regex)))
            .map(|(start, end)| program::FilterRange::new(start, end))
            .map(From::from),
    );

    let lines = command(
        &["lines", "line", "l"],
        next.and_then(arg(all_consuming(separated_list0(char(','), lines_atom))
            .map(program::Lines::new)
            .map(From::from))),
    );

    let fields = command(
        &["fields", "F"],
        next.and_then(arg(all_consuming(separated_list0(char(','), fields_atom))
            .map(program::Fields::new)
            .map(From::from))),
    );

    Ok(all_consuming(many0(alt((
        enumeration,
        fields,
        filter,
        lines,
        filter_range,
        gsub,
        match_,
        match_range,
        sub,
        |i: Args| match i.first() {
            Some(arg) => Err(nom::Err::Failure(ParseError::msg(format!(
                "Not a recognized keyword: {}",
                arg
            )))),
            None => Err(nom::Err::Error(ParseError::new())),
        },
    ))))
    .parse(input)
    .finish()
    .map(|(_, vec)| vec)
    .map(Program::new)?)
}

fn next(input: Args) -> IResult<Args, &str> {
    input
        .split_first()
        .map(|(first, rest)| (rest, *first))
        .ok_or(nom::Err::Error(ParseError::msg("Missing argument")))
}

fn command<'i>(
    matches: Args<'static>,
    parser: impl Parser<Args<'i>, Atom, ParseError>,
) -> impl FnMut(Args<'i>) -> IResult<Args<'i>, Atom> {
    let mut inner = cut(parser);
    move |input| {
        // nom's flat_map doesn't work here since its closure would consume the parser
        let (input, arg) = verify(next, move |item| matches.contains(item)).parse(input)?;
        inner
            .parse(input)
            .context(format!("Failed parsing arguments of '{}'", arg))
    }
}

// Wraps parser to provide an 'Invalid argument' context
fn arg<I, O>(parser: impl Parser<I, O, ParseError>) -> impl Parser<I, O, ParseError>
where
    I: Clone + Display,
{
    add_context(|i| format!("Invalid argument: {}", i), parser)
}

// Consumes the whole input or errors
fn regex<'s>(input: &'s str) -> IResult<&'s str, Regex> {
    Regex::new(input).map(|res| ("", res)).map_err(|err| {
        nom::Err::Error(ParseError::Message(match err {
            regex::Error::Syntax(m) => anyhow::Error::msg(m),
            _ => anyhow::Error::msg(format!("Invalid regular expression: {}", input)),
        }))
    })
}

fn usize(s: &str) -> IResult<&str, usize> {
    map_res(digit1, |s: &str| s.parse::<usize>()).parse(s)
}

fn field_id(s: &str) -> IResult<&str, program::FieldId> {
    alt((
        usize.map(program::FieldId::Int),
        delimited(char('('), preceded(char('-'), usize), char(')')).map(program::FieldId::FromLast),
    ))
    .parse(s)
}

fn fields_atom(s: &str) -> IResult<&str, program::FieldsAtom> {
    alt((
        separated_pair(opt(field_id), char('-'), opt(field_id))
            .map(|(opt1, opt2)| program::OpenRange::new(opt1, opt2))
            .map(program::FieldsAtom::Range),
        field_id.map(program::FieldsAtom::Single),
    ))
    .parse(s)
}

fn lines_atom(s: &str) -> IResult<&str, program::LinesAtom> {
    alt((
        separated_pair(opt(usize), char('-'), opt(usize))
            .map(|(opt1, opt2)| program::OpenRange::new(opt1, opt2))
            .map(program::LinesAtom::Range),
        usize.map(program::LinesAtom::Single),
    ))
    .parse(s)
}
