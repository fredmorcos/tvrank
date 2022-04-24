#![warn(clippy::all)]
#![warn(missing_docs)]

//! TVrank-imdb is a library for querying and ranking information about movies and series
//! using the IMDB dataset (TSV dumps) as a source.

mod db;
mod error;
mod genre;
mod ratings;
mod service;
mod title;
mod title_header;
mod title_id;
mod title_type;
mod tokens;

pub use db::Query as ImdbQuery;
pub use error::Err as ImdbErr;
pub use genre::{Genre as ImdbGenre, Genres as ImdbGenres};
pub use service::Service as Imdb;
pub use title::Title as ImdbTitle;
pub use title_id::TitleId as ImdbTitleId;
pub use title_type::TitleType as ImdbTitleType;
