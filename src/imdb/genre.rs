#![warn(clippy::all)]

use derive_more::Display;
use enum_utils::FromStr;
use std::fmt;

#[derive(Debug, Display, FromStr, PartialEq, Eq, Hash, Clone, Copy)]
#[display(fmt = "{}")]
pub enum Genre {
  Drama = 0,
  Documentary,
  Short,
  Animation,
  Comedy,
  Sport,
  Fantasy,
  Horror,
  Romance,
  News,
  Biography,
  Music,
  Musical,
  War,
  Crime,
  Western,
  Family,
  Adventure,
  Action,
  History,
  Mystery,
  Thriller,
  Adult,
  #[display(fmt = "Reality-TV")]
  #[enumeration(rename = "Reality-TV")]
  RealityTv,
  #[display(fmt = "Sci-Fi")]
  #[enumeration(rename = "Sci-Fi")]
  SciFi,
  #[display(fmt = "Film-Noir")]
  #[enumeration(rename = "Film-Noir")]
  FilmNoir,
  #[display(fmt = "Talk-Show")]
  #[enumeration(rename = "Talk-Show")]
  TalkShow,
  #[display(fmt = "Game-Show")]
  #[enumeration(rename = "Game-Show")]
  GameShow,
}

#[derive(PartialEq, Eq, Default, Clone, Copy)]
pub struct Genres(u64);

impl Genres {
  pub(crate) fn add_genre(&mut self, genre: Genre) {
    let index = genre as u8;
    self.0 |= 1 << index;
  }
}

impl fmt::Debug for Genres {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    <Self as fmt::Display>::fmt(self, f)
  }
}

impl fmt::Display for Genres {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut first = true;

    for index in 0..u64::BITS {
      let index = index as u8;

      if (self.0 >> index) & 1 == 1 {
        let genre: Genre = unsafe { std::mem::transmute(index) };

        if first {
          write!(f, "{}", genre)?;
          first = false;
        } else {
          write!(f, ", {}", genre)?;
        }
      }
    }

    Ok(())
  }
}
