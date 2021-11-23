#![warn(clippy::all)]

use crate::imdb::genre::Genres;
use deepsize::DeepSizeOf;
use derive_more::{Display, From, Into};
use derive_new::new;
use enum_utils::FromStr;
use std::{cmp::Ordering, time::Duration};

#[derive(Debug, Display, FromStr, PartialEq, Eq, Hash, Clone, Copy, DeepSizeOf)]
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

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, Into, DeepSizeOf)]
pub struct TitleId(u64);

impl PartialOrd for TitleId {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    self.0.partial_cmp(&other.0)
  }
}

impl Ord for TitleId {
  fn cmp(&self, other: &Self) -> Ordering {
    self.0.cmp(&other.0)
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, new, DeepSizeOf)]
pub struct Title {
  title_id: TitleId,
  title_type: TitleType,
  is_adult: bool,
  start_year: Option<u16>,
  end_year: Option<u16>,
  runtime_minutes: Option<u16>,
  genres: Genres,
}

impl Title {
  pub fn title_id(&self) -> TitleId {
    self.title_id
  }

  pub fn title_type(&self) -> TitleType {
    self.title_type
  }

  pub fn start_year(&self) -> Option<u16> {
    self.start_year
  }

  pub fn end_year(&self) -> Option<u16> {
    self.end_year
  }

  pub fn runtime(&self) -> Option<Duration> {
    self.runtime_minutes.map(|runtime| Duration::from_secs(u64::from(runtime) * 60))
  }

  pub fn genres(&self) -> Genres {
    self.genres
  }
}
