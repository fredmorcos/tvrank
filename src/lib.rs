#![warn(clippy::all)]

use std::error::Error;

pub type Res<T> = Result<T, Box<dyn Error>>;

mod io;
mod mem;
mod progressbar;
mod utils;

pub mod imdb;
