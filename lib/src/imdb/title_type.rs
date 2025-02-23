#![warn(clippy::all)]

use derive_more::Display;
use serde::Serialize;
use std::{hash::Hash, str::FromStr};

/// Encodes the 13 types of a title.
#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
pub enum TitleType {
  // Games
  /// VideoGame.
  VideoGame = 0,

  // Movies
  /// Short.
  #[display("Short Movie")]
  Short = 1,
  /// Video.
  Video = 2,
  /// Movie.
  Movie = 3,
  /// TvShort.
  #[display("TV Short")]
  TvShort = 4,
  /// TvMovie.
  #[display("TV Movie")]
  TvMovie = 5,
  /// TvSpecial.
  #[display("TV Special")]
  TvSpecial = 6,

  // Episodes
  /// TvEpisode.
  TvEpisode = 7,
  /// TvPilot.
  TvPilot = 8,
  /// RadioEpisode.
  RadioEpisode = 9,

  // Series
  /// TvSeries.
  #[display("TV Series")]
  TvSeries = 10,
  /// TvMiniSeries.
  #[display("TV Mini-Series")]
  TvMiniSeries = 11,

  // Radio
  /// RadioSeries.
  #[display("Radio Series")]
  RadioSeries = 12,
}

impl FromStr for TitleType {
  type Err = ();

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "videoGame" => Ok(TitleType::VideoGame),
      "short" => Ok(TitleType::Short),
      "video" => Ok(TitleType::Video),
      "movie" => Ok(TitleType::Movie),
      "tvShort" => Ok(TitleType::TvShort),
      "tvMovie" => Ok(TitleType::TvMovie),
      "tvSpecial" => Ok(TitleType::TvSpecial),
      "tvEpisode" => Ok(TitleType::TvEpisode),
      "tvPilot" => Ok(TitleType::TvPilot),
      "radioEpisode" => Ok(TitleType::RadioEpisode),
      "tvSeries" => Ok(TitleType::TvSeries),
      "tvMiniSeries" => Ok(TitleType::TvMiniSeries),
      "radioSeries" => Ok(TitleType::RadioSeries),
      _ => Err(()),
    }
  }
}

impl TitleType {
  /// Converts a byte representation to its corresponding TitleType.
  ///
  /// # Arguments
  ///
  /// * `value` - Byte representation of a TitleType.
  pub(crate) const fn from(value: u8) -> Self {
    unsafe { std::mem::transmute(value) }
  }

  /// Whether the title type refers to a movie.
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
      TitleType::TvSeries | TitleType::TvMiniSeries => false,

      // Radio
      TitleType::RadioSeries => false,
    }
  }

  /// Whether the title type refers to a series.
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
      TitleType::TvSeries | TitleType::TvMiniSeries => true,

      // Radio
      TitleType::RadioSeries => false,
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::imdb::title_type::TitleType;

  #[test]
  fn test_title_type_value() {
    assert_eq!(TitleType::VideoGame as u8, 0);
    assert_eq!(TitleType::Short as u8, 1);
    assert_eq!(TitleType::Video as u8, 2);
    assert_eq!(TitleType::Movie as u8, 3);
    assert_eq!(TitleType::TvShort as u8, 4);
    assert_eq!(TitleType::TvMovie as u8, 5);
    assert_eq!(TitleType::TvSpecial as u8, 6);
    assert_eq!(TitleType::TvEpisode as u8, 7);
    assert_eq!(TitleType::TvPilot as u8, 8);
    assert_eq!(TitleType::RadioEpisode as u8, 9);
    assert_eq!(TitleType::TvSeries as u8, 10);
    assert_eq!(TitleType::TvMiniSeries as u8, 11);
    assert_eq!(TitleType::RadioSeries as u8, 12);
  }

  #[test]
  fn test_title_type_from_u8() {
    assert_eq!(TitleType::VideoGame, TitleType::from(0));
    assert_eq!(TitleType::Short, TitleType::from(1));
    assert_eq!(TitleType::Video, TitleType::from(2));
    assert_eq!(TitleType::Movie, TitleType::from(3));
    assert_eq!(TitleType::TvShort, TitleType::from(4));
    assert_eq!(TitleType::TvMovie, TitleType::from(5));
    assert_eq!(TitleType::TvSpecial, TitleType::from(6));
    assert_eq!(TitleType::TvEpisode, TitleType::from(7));
    assert_eq!(TitleType::TvPilot, TitleType::from(8));
    assert_eq!(TitleType::RadioEpisode, TitleType::from(9));
    assert_eq!(TitleType::TvSeries, TitleType::from(10));
    assert_eq!(TitleType::TvMiniSeries, TitleType::from(11));
    assert_eq!(TitleType::RadioSeries, TitleType::from(12));
  }

  #[test]
  fn test_is_movie() {
    assert!(!TitleType::from(0).is_movie());
    assert!(TitleType::from(1).is_movie());
    assert!(TitleType::from(2).is_movie());
    assert!(TitleType::from(3).is_movie());
    assert!(TitleType::from(4).is_movie());
    assert!(TitleType::from(5).is_movie());
    assert!(TitleType::from(6).is_movie());
    assert!(!TitleType::from(7).is_movie());
    assert!(!TitleType::from(8).is_movie());
    assert!(!TitleType::from(9).is_movie());
    assert!(!TitleType::from(10).is_movie());
    assert!(!TitleType::from(11).is_movie());
    assert!(!TitleType::from(12).is_movie());
  }

  #[test]
  fn test_is_series() {
    assert!(!TitleType::from(0).is_series());
    assert!(!TitleType::from(1).is_series());
    assert!(!TitleType::from(2).is_series());
    assert!(!TitleType::from(3).is_series());
    assert!(!TitleType::from(4).is_series());
    assert!(!TitleType::from(5).is_series());
    assert!(!TitleType::from(6).is_series());
    assert!(!TitleType::from(7).is_series());
    assert!(!TitleType::from(8).is_series());
    assert!(!TitleType::from(9).is_series());
    assert!(TitleType::from(10).is_series());
    assert!(TitleType::from(11).is_series());
    assert!(!TitleType::from(12).is_series());
  }
}
