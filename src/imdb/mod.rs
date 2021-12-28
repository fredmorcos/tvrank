#![warn(clippy::all)]

mod basics;
mod error;
mod genre;
mod parsing;
mod ratings;
mod service;
mod storage;
mod title;

pub use basics::QueryType as ImdbQueryType;
pub use error::Err as ImdbErr;
pub use genre::{Genre as ImdbGenre, Genres as ImdbGenres};
pub use service::Service as Imdb;
pub use storage::Storage as ImdbStorage;
pub use title::{Title as ImdbTitle, TitleId as ImdbTitleId, TitleType as ImdbTitleType};
