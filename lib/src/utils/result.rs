#![warn(clippy::all)]

//! Common types and utilities for error-handling.

use std::error::Error;

/// A shorthand type for library functions that may return errors.
pub type Res<T = ()> = Result<T, Box<dyn Error>>;
