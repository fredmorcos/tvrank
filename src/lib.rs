#![warn(clippy::all)]

use std::error::Error;

pub type Res<T> = Result<T, Box<dyn Error>>;

pub mod imdb;
pub mod io;
pub mod mem;
pub mod progressbar;
pub mod utils;
