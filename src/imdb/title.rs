#![warn(clippy::all)]

use super::error::Err;
use super::genre::{Genre, Genres};
use atoi::atoi;
use derive_more::Display;
use enum_utils::FromStr;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Display, FromStr, PartialEq, Eq, Hash, Clone, Copy)]
#[enumeration(rename_all = "camelCase")]
#[display(fmt = "{}")]
pub enum TitleType {
  // Games
  VideoGame,

  // Movies
  #[display(fmt = "Short Movie")]
  Short,
  Video,
  Movie,
  #[display(fmt = "TV Short")]
  TvShort,
  #[display(fmt = "TV Movie")]
  TvMovie,
  #[display(fmt = "TV Special")]
  TvSpecial,

  // Episodes
  TvEpisode,
  TvPilot,
  RadioEpisode,

  // Series
  #[display(fmt = "TV Series")]
  TvSeries,
  #[display(fmt = "TV Mini-Series")]
  TvMiniSeries,

  // Radio
  #[display(fmt = "Radio Series")]
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
      TitleType::TvSeries | TitleType::TvMiniSeries => false,

      // Radio
      TitleType::RadioSeries => false,
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
      TitleType::TvSeries | TitleType::TvMiniSeries => true,

      // Radio
      TitleType::RadioSeries => false,
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub struct TitleId<'a> {
  bytes: &'a [u8],
  num: usize,
}

impl PartialEq for TitleId<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.num == other.num
  }
}

impl Eq for TitleId<'_> {}

impl Hash for TitleId<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.num.hash(state);
  }
}

impl<'a> TitleId<'a> {
  pub(crate) fn as_str(&self) -> &'a str {
    unsafe { std::str::from_utf8_unchecked(self.bytes) }
  }

  pub(crate) fn as_usize(&self) -> usize {
    self.num
  }
}

impl<'a> TryFrom<&'a [u8]> for TitleId<'a> {
  type Error = Box<dyn Error>;

  fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
    if &bytes[0..=1] != super::parsing::TT {
      return Err::id(unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned());
    }

    let num = atoi::<usize>(&bytes[2..])
      .ok_or_else(|| Err::IdNumber(unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned()))?;

    Ok(TitleId { bytes, num })
  }
}

impl fmt::Display for TitleId<'_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

#[derive(Debug, Clone)]
pub(crate) struct TitleBasics {
  pub(crate) title_id: TitleId<'static>,
  pub(crate) title_type: TitleType,
  pub(crate) primary_title: &'static str,
  pub(crate) original_title: &'static str,
  pub(crate) is_adult: bool,
  pub(crate) start_year: Option<u16>,
  pub(crate) end_year: Option<u16>,
  pub(crate) runtime_minutes: Option<u16>,
  pub(crate) genres: Genres,
}

impl TryFrom<&'static [u8]> for TitleBasics {
  type Error = Box<dyn Error>;

  fn try_from(line: &'static [u8]) -> Result<Self, Self::Error> {
    let mut iter = line.split(|&b| b == super::parsing::TAB);

    macro_rules! next {
      () => {{
        iter.next().ok_or(Err::Eof)?
      }};
    }

    let title_id = TitleId::try_from(next!())?;

    let title_type = {
      let title_type = next!();
      let title_type = unsafe { std::str::from_utf8_unchecked(title_type) };
      TitleType::from_str(title_type).map_err(|_| Err::TitleType)?
    };

    if !title_type.is_movie() && !title_type.is_series() {
      return Err(Box::new(Err::UnsupportedTitleType));
    }

    let primary_title = unsafe { std::str::from_utf8_unchecked(next!()) };
    let original_title = unsafe { std::str::from_utf8_unchecked(next!()) };

    let is_adult = {
      let is_adult = next!();
      match is_adult {
        super::parsing::ZERO => false,
        super::parsing::ONE => true,
        _ => return Err::adult(),
      }
    };

    let start_year = {
      let start_year = next!();
      match start_year {
        super::parsing::NOT_AVAIL => None,
        start_year => Some(atoi::<u16>(start_year).ok_or(Err::StartYear)?),
      }
    };

    let end_year = {
      let end_year = next!();
      match end_year {
        super::parsing::NOT_AVAIL => None,
        end_year => Some(atoi::<u16>(end_year).ok_or(Err::EndYear)?),
      }
    };

    let runtime_minutes = {
      let runtime_minutes = next!();
      match runtime_minutes {
        super::parsing::NOT_AVAIL => None,
        runtime_minutes => Some(atoi::<u16>(runtime_minutes).ok_or(Err::RuntimeMinutes)?),
      }
    };

    let genres = {
      let genres = next!();
      let mut result = Genres::default();

      if genres != super::parsing::NOT_AVAIL {
        let genres = genres.split(|&b| b == super::parsing::COMMA);
        for genre in genres {
          let genre = unsafe { std::str::from_utf8_unchecked(genre) };
          let genre = Genre::from_str(genre).map_err(|_| Err::Genre)?;
          result.add(genre);
        }
      }

      result
    };

    Ok(TitleBasics {
      title_id,
      title_type,
      primary_title,
      original_title,
      is_adult,
      start_year,
      end_year,
      runtime_minutes,
      genres,
    })
  }
}

impl PartialEq for TitleBasics {
  fn eq(&self, other: &Self) -> bool {
    self.title_id == other.title_id
  }
}

impl Eq for TitleBasics {}

impl Hash for TitleBasics {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.title_id.hash(state);
  }
}

#[derive(Debug, Clone)]
pub struct Title<'basics, 'ratings> {
  basics: &'basics TitleBasics,
  rating: Option<&'ratings (u8, u64)>,
}

impl PartialEq for Title<'_, '_> {
  fn eq(&self, other: &Self) -> bool {
    self.basics == other.basics
  }
}

impl Eq for Title<'_, '_> {}

impl<'basics, 'ratings> Title<'basics, 'ratings> {
  pub(crate) fn new(basics: &'basics TitleBasics, rating: Option<&'ratings (u8, u64)>) -> Self {
    Self { basics, rating }
  }

  pub fn title_id(&self) -> &TitleId {
    &self.basics.title_id
  }

  pub fn title_type(&self) -> TitleType {
    self.basics.title_type
  }

  pub fn primary_title(&self) -> &str {
    self.basics.primary_title
  }

  pub fn original_title(&self) -> Option<&str> {
    if self.basics.original_title.to_lowercase() == self.basics.primary_title.to_lowercase() {
      None
    } else {
      Some(self.basics.original_title)
    }
  }

  pub fn is_adult(&self) -> bool {
    self.basics.is_adult
  }

  pub fn start_year(&self) -> Option<u16> {
    self.basics.start_year
  }

  pub fn end_year(&self) -> Option<u16> {
    self.basics.end_year
  }

  pub fn runtime(&self) -> Option<Duration> {
    self
      .basics
      .runtime_minutes
      .map(|runtime| Duration::from_secs(u64::from(runtime) * 60))
  }

  pub fn genres(&self) -> Genres {
    self.basics.genres
  }

  pub fn rating(&self) -> Option<&(u8, u64)> {
    self.rating
  }
}
