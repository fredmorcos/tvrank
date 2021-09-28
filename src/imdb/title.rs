#![warn(clippy::all)]

use crate::imdb::genre::Genres;
use derive_more::Display;
use derive_new::new;
use enum_utils::FromStr;
use std::fmt;

#[derive(Debug, Display, FromStr, PartialEq, Eq, Hash, Clone, Copy)]
#[enumeration(rename_all = "camelCase")]
#[display(fmt = "{}")]
pub enum TitleType {
  // Games
  VideoGame,

  // Movies
  Short,
  Video,
  Movie,
  TvShort,
  TvMovie,
  TvSpecial,

  // Episodes
  TvEpisode,
  TvPilot,
  RadioEpisode,

  // Series
  TvSeries,
  TvMiniSeries,
  RadioSeries,
}

impl TitleType {
  pub(crate) fn is_movie(&self) -> bool {
    match self {
      // Games
      TitleType::VideoGame => false,

      // Movies
      TitleType::Short
      | TitleType::Video
      | TitleType::Movie
      | TitleType::TvShort
      | TitleType::TvMovie
      | TitleType::TvSpecial => true,

      // Episodes
      TitleType::TvEpisode | TitleType::TvPilot | TitleType::RadioEpisode => false,

      // Series
      TitleType::TvSeries | TitleType::TvMiniSeries | TitleType::RadioSeries => false,
    }
  }

  pub(crate) fn is_series(&self) -> bool {
    match self {
      // Games
      TitleType::VideoGame => false,

      // Movies
      TitleType::Short
      | TitleType::Video
      | TitleType::Movie
      | TitleType::TvShort
      | TitleType::TvMovie
      | TitleType::TvSpecial => false,

      // Episodes
      TitleType::TvEpisode | TitleType::TvPilot | TitleType::RadioEpisode => false,

      // Series
      TitleType::TvSeries | TitleType::TvMiniSeries | TitleType::RadioSeries => true,
    }
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, new)]
pub struct Title {
  title_type: TitleType,
  is_adult: bool,
  start_year: Option<u16>,
  end_year: Option<u16>,
  runtime_minutes: Option<u16>,
  genres: Genres,
  #[new(default)]
  average_rating: u8,
  #[new(default)]
  num_votes: u64,
}

impl fmt::Display for Title {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.title_type)?;

    if let Some(year) = self.start_year {
      write!(f, " ({})", year)?;
    }

    write!(f, " [{}]", self.genres)
  }
}
