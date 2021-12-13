#![warn(clippy::all)]

mod parsing;
mod storage;

pub mod basics;
pub mod error;
pub mod genre;
pub mod keywords;
pub mod ratings;
pub mod service;
pub mod title;

pub use error::Err as ImdbErr;
pub use genre::{Genre as ImdbGenre, Genres as ImdbGenres};
pub use keywords::KeywordSet as ImdbKeywordSet;
pub use service::{QueryType as ImdbQueryType, Service as Imdb};
pub use storage::Storage as ImdbStorage;
pub use title::{Title as ImdbTitle, TitleId as ImdbTitleId, TitleType as ImdbTitleType};
