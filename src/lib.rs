#![warn(clippy::all)]

//! TVrank is a library for querying and ranking information about movies and series.
//! It can be used to query a single title or scan media directories.

use std::error::Error;

/// A type alias for Result<T, Box<dyn Error>>
pub type Res<T> = Result<T, Box<dyn Error>>;

pub mod imdb;
