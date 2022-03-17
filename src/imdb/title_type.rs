#![warn(clippy::all)]

use derive_more::Display;
use enum_utils::FromStr;
use std::hash::Hash;

/// Encodes the 13 types a title can be
#[derive(Debug, Display, FromStr, PartialEq, Eq, Hash, Clone, Copy)]
#[enumeration(rename_all = "camelCase")]
#[display(fmt = "{}")]
pub enum TitleType {
  // Games
  VideoGame = 0,

  // Movies
  #[display(fmt = "Short Movie")]
  Short = 1,
  Video = 2,
  Movie = 3,
  #[display(fmt = "TV Short")]
  TvShort = 4,
  #[display(fmt = "TV Movie")]
  TvMovie = 5,
  #[display(fmt = "TV Special")]
  TvSpecial = 6,

  // Episodes
  TvEpisode = 7,
  TvPilot = 8,
  RadioEpisode = 9,

  // Series
  #[display(fmt = "TV Series")]
  TvSeries = 10,
  #[display(fmt = "TV Mini-Series")]
  TvMiniSeries = 11,

  // Radio
  #[display(fmt = "Radio Series")]
  RadioSeries = 12,
}

impl TitleType {
  // pub(crate) const fn max() -> u8 {
  //   Self::RadioSeries as u8
  // }

  /// Converts a byte representation to its corresponding TitleType
  /// # Arguments
  /// * `value` - Byte representation of a TitleType
  pub(crate) const unsafe fn from(value: u8) -> Self {
    std::mem::transmute(value)
  }

  /// Returns true if the TitleType is movie
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

  /// Returns true if the TitleType is series
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
  use super::TitleType;

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
    assert_eq!(TitleType::VideoGame, unsafe { TitleType::from(0) });
    assert_eq!(TitleType::Short, unsafe { TitleType::from(1) });
    assert_eq!(TitleType::Video, unsafe { TitleType::from(2) });
    assert_eq!(TitleType::Movie, unsafe { TitleType::from(3) });
    assert_eq!(TitleType::TvShort, unsafe { TitleType::from(4) });
    assert_eq!(TitleType::TvMovie, unsafe { TitleType::from(5) });
    assert_eq!(TitleType::TvSpecial, unsafe { TitleType::from(6) });
    assert_eq!(TitleType::TvEpisode, unsafe { TitleType::from(7) });
    assert_eq!(TitleType::TvPilot, unsafe { TitleType::from(8) });
    assert_eq!(TitleType::RadioEpisode, unsafe { TitleType::from(9) });
    assert_eq!(TitleType::TvSeries, unsafe { TitleType::from(10) });
    assert_eq!(TitleType::TvMiniSeries, unsafe { TitleType::from(11) });
    assert_eq!(TitleType::RadioSeries, unsafe { TitleType::from(12) });
  }

  // #[test]
  // fn test_title_type_max() {
  //   assert_eq!(TitleType::max(), TitleType::RadioSeries as u8);
  // }
}
