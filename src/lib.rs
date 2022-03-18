#![warn(clippy::all)]
#![warn(missing_docs)]

//! TVrank is a library for querying and ranking information about movies and series.
//! It can be used to query a single title or scan media directories.

use std::error::Error;

/// A shorthand type for library functions that may return errors
pub type Res<T> = Result<T, Box<dyn Error>>;

/// Provides functionality to download and store titles from IMDb and allows the users to query this data.  
pub mod imdb;
