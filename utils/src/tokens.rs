#![warn(clippy::all)]

//! Common utilities for writing parsers.

/// Get the next iterator item or propagate an EOF error if the end is reached.
///
/// This macro can be used when writing parsers that expect more input to be consumed
/// and it is considered an unexpected EOF error if there isn't any more input.
///
/// # Arguments
///
/// * `($iter: ident)` - The identifier of an iterator.
///
/// # Errors
///
/// * `Err::Eof` - Propagate an unexpected EOF error up the stack when the end of the
/// iterator is reached. This should be expected behavior since this macro is used when
/// a parser is expected to consume more input.
#[macro_export]
macro_rules! iter_next {
  ($iter:ident) => {{
    $iter.next().ok_or(Err::Eof)?
  }};
}
