//! Error representations, primarily related to parsing failure.

use crate::data::Span;

/// Represents a failure in parsing at some point in the combinator chain.
pub type ParseError<'a> = nom::Err<nom::error::Error<Span<'a>>>;
