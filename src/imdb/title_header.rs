#![warn(clippy::all)]

use crate::imdb::genre::Genres;
use crate::imdb::ratings::Rating;
use crate::imdb::title_type::TitleType;
use std::ops::Deref;

/// # Header version 0 is 16 bytes composed of (from MSB to LSB):
///
/// * 1 byte:
///   * Version:                6  bits
///   * Has an Original Title:  1  bit
///   * Is Adult:               1  bit
///
/// * 2 bytes:
///   * Runtime in Minutes:     16 bits (value 0 means unknown)
///
/// * 2 bytes:
///   * Year:                   9  bits (starts at year 1800: 1800 + year value; 0 means unknown)
///   * Rating:                 7  bits (if rating is 0, check the number of votes)
///
/// * 4 bytes:
///   * Number of Rating Votes: 23 bits
///   * Title Type:             5  bits
///
/// * 4 bytes:
///   * Genres:                 32 bits
///
/// * 3 bytes are reserved for later version use.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) struct TitleHeader(u128);

impl Deref for TitleHeader {
  type Target = u128;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<[u8; 16]> for TitleHeader {
  fn from(bytes: [u8; 16]) -> Self {
    TitleHeader(u128::from_le_bytes(bytes))
  }
}

impl TitleHeader {
  pub(crate) fn new_version_0(
    has_original_title: bool,
    is_adult: bool,
    runtime_minutes: Option<u16>,
    start_year: Option<u16>,
    rating: Option<Rating>,
    title_type: TitleType,
    genres: Genres,
  ) -> Self {
    let version = 0;
    let has_original_title = if has_original_title {
      1
    } else {
      0
    };
    let is_adult = if is_adult {
      1
    } else {
      0
    };

    let runtime = if let Some(runtime) = runtime_minutes {
      u128::from(runtime)
    } else {
      0
    };

    let year = if let Some(year) = start_year {
      assert!(year > 1800);
      u128::from(year - 1800)
    } else {
      0
    };
    let (rating, votes) = if let Some(rating) = rating {
      (u128::from(rating.rating()), u128::from(rating.votes()))
    } else {
      (0, 0)
    };

    let title_type = u128::from(title_type as u8);

    let genres = u128::from(u32::from(genres));

    let header = version
      | (has_original_title << 6)
      | (is_adult << 7)
      | (runtime << 8)
      | (year << 24)
      | (rating << 33)
      | (votes << 40)
      | (title_type << 63)
      | (genres << 68);

    Self(header)
  }

  // fn version(&self) -> u8 {
  //   let mask = 2_u128.pow(6) - 1;
  //   (self.0 & mask) as u8
  // }

  pub(crate) fn has_original_title(&self) -> bool {
    let mask = 1 << 6;
    ((self.0 & mask) >> 6) == 1
  }

  pub(crate) fn is_adult(&self) -> bool {
    let mask = 1 << 7;
    ((self.0 & mask) >> 7) == 1
  }

  pub(crate) fn runtime_minutes(&self) -> Option<u16> {
    let mask = (2_u128.pow(16) - 1) << 8;
    let value = (self.0 & mask) >> 8;
    if value == 0 {
      None
    } else {
      Some(value as u16)
    }
  }

  pub(crate) fn start_year(&self) -> Option<u16> {
    let mask = (2_u128.pow(9) - 1) << 24;
    let value = (self.0 & mask) >> 24;
    if value == 0 {
      None
    } else {
      Some(1800 + value as u16)
    }
  }

  pub(crate) fn rating(&self) -> Option<Rating> {
    let mask = (2_u128.pow(7) - 1) << 33;
    let rating = (self.0 & mask) >> 33;
    let mask = (2_u128.pow(23) - 1) << 40;
    let votes = (self.0 & mask) >> 40;

    if votes == 0 {
      None
    } else {
      Some(Rating::new(rating as u8, votes as u32))
    }
  }

  pub(crate) fn title_type(&self) -> TitleType {
    let mask = (2_u128.pow(5) - 1) << 63;
    let value = (self.0 & mask) >> 63;
    unsafe { TitleType::from(value as u8) }
  }

  pub(crate) fn genres(&self) -> Genres {
    let mask = (2_u128.pow(32) - 1) << 68;
    let value = (self.0 & mask) >> 68;
    Genres::from(value as u32)
  }
}
