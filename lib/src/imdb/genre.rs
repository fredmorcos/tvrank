#![warn(clippy::all)]

use derive_more::Display;
use enum_utils::FromStr;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use std::fmt;

/// 27 genres a title can be associated with
#[derive(Debug, Display, FromStr, PartialEq, Eq, Hash, Clone, Copy, Serialize)]
#[display(fmt = "{}")]
pub enum Genre {
  /// Action
  Action = 0,
  /// Adult
  Adult = 1,
  /// Adventure
  Adventure = 2,
  /// Animation
  Animation = 3,
  /// Biography
  Biography = 4,
  /// Comedy
  Comedy = 5,
  /// Crime
  Crime = 6,
  /// Documentary
  Documentary = 7,
  /// Drama
  Drama = 8,
  /// Family
  Family = 9,
  /// Fantasy
  Fantasy = 10,
  /// FilmNoir
  #[display(fmt = "Film-Noir")]
  #[enumeration(rename = "Film-Noir")]
  #[serde(rename(serialize = "Film-Noir"))]
  FilmNoir = 11,
  /// GameShow
  #[display(fmt = "Game-Show")]
  #[enumeration(rename = "Game-Show")]
  #[serde(rename(serialize = "Game-Show"))]
  GameShow = 12,
  /// History
  History = 13,
  /// Horror
  Horror = 14,
  /// Music
  Music = 15,
  /// Musical
  Musical = 16,
  /// Mystery
  Mystery = 17,
  /// News
  News = 18,
  /// RealityTv
  #[display(fmt = "Reality-TV")]
  #[enumeration(rename = "Reality-TV")]
  #[serde(rename(serialize = "Reality-TV"))]
  RealityTv = 19,
  /// Romance
  Romance = 20,
  /// SciFi
  #[display(fmt = "Sci-Fi")]
  #[enumeration(rename = "Sci-Fi")]
  #[serde(rename(serialize = "Sci-Fi"))]
  SciFi = 21,
  /// Short
  Short = 22,
  /// Sport
  Sport = 23,
  /// TalkShow
  #[display(fmt = "Talk-Show")]
  #[enumeration(rename = "Talk-Show")]
  #[serde(rename(serialize = "Talk-Show"))]
  TalkShow = 24,
  /// Thriller
  Thriller = 25,
  /// War
  War = 26,
  /// Western
  Western = 27,
  /// Experimental
  Experimental = 28,
}

impl Genre {
  /// Returns the largest-valued [Genre] enum variant enum as [u8].
  pub(crate) const fn max() -> u8 {
    Self::Experimental as u8
  }

  /// Converts a number into its corresponding Genre item.
  ///
  /// # Arguments
  ///
  /// * `value` - u8 value to be converted to a Genre item.
  pub(crate) const unsafe fn from(value: u8) -> Self {
    std::mem::transmute(value)
  }
}

/// Represents the set of genres a title is associated with
#[derive(PartialEq, Eq, Default, Clone, Copy)]
pub struct Genres(u32);

impl Genres {
  /// Add a new Genre into the Genres.
  ///
  /// # Arguments
  ///
  /// * `genre` - Genre to be added to the Genres.
  pub(crate) fn add(&mut self, genre: Genre) {
    let index = genre as u8;
    self.0 |= 1 << index;
  }

  /// Returns an iterator for the genres.
  pub fn iter(&self) -> GenresIter {
    GenresIter::new(*self)
  }

  /// Returns the item at the given index in the Genre enum.
  ///
  /// # Arguments
  ///
  /// * `index` - Index of the item to be returned.
  fn get(&self, index: u8) -> Option<Genre> {
    if index > Genre::max() {
      panic!("Genre index `{}` out of range (max: {})", index, Genre::max());
    }

    if (self.0 >> index) & 1 == 1 {
      Some(unsafe { Genre::from(index) })
    } else {
      None
    }
  }
}

impl Serialize for Genres {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut seq = serializer.serialize_seq(None)?;
    for e in self.iter() {
      seq.serialize_element(&e)?;
    }
    seq.end()
  }
}

impl From<Genres> for u32 {
  fn from(genres: Genres) -> Self {
    genres.0
  }
}

impl From<u32> for Genres {
  fn from(values: u32) -> Self {
    Genres(values)
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

    for genre in self.iter() {
      if first {
        write!(f, "{genre}")?;
        first = false;
      } else {
        write!(f, ", {genre}")?;
      }
    }

    Ok(())
  }
}

/// Iterator for the Genres struct.
pub struct GenresIter {
  genres: Genres,
  index: u8,
}

impl GenresIter {
  /// Creates and returns a GenresIter.
  ///
  /// # Arguments
  ///
  /// * `genres` - Genres struct to get the iterator.
  pub fn new(genres: Genres) -> Self {
    Self { genres, index: 0 }
  }
}

impl Iterator for GenresIter {
  type Item = Genre;

  /// Get the next item in the GenresIter.
  fn next(&mut self) -> Option<Self::Item> {
    loop {
      if self.index > Genre::max() {
        return None;
      }

      if let Some(genre) = self.genres.get(self.index) {
        self.index += 1;
        return Some(genre);
      }

      self.index += 1;
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::imdb::genre::Genre;
  use crate::imdb::genre::Genres;

  #[test]
  fn test_genre_value() {
    assert_eq!(Genre::Action as u8, 0);
    assert_eq!(Genre::Adult as u8, 1);
    assert_eq!(Genre::Adventure as u8, 2);
    assert_eq!(Genre::Animation as u8, 3);
    assert_eq!(Genre::Biography as u8, 4);
    assert_eq!(Genre::Comedy as u8, 5);
    assert_eq!(Genre::Crime as u8, 6);
    assert_eq!(Genre::Documentary as u8, 7);
    assert_eq!(Genre::Drama as u8, 8);
    assert_eq!(Genre::Family as u8, 9);
    assert_eq!(Genre::Fantasy as u8, 10);
    assert_eq!(Genre::FilmNoir as u8, 11);
    assert_eq!(Genre::GameShow as u8, 12);
    assert_eq!(Genre::History as u8, 13);
    assert_eq!(Genre::Horror as u8, 14);
    assert_eq!(Genre::Music as u8, 15);
    assert_eq!(Genre::Musical as u8, 16);
    assert_eq!(Genre::Mystery as u8, 17);
    assert_eq!(Genre::News as u8, 18);
    assert_eq!(Genre::RealityTv as u8, 19);
    assert_eq!(Genre::Romance as u8, 20);
    assert_eq!(Genre::SciFi as u8, 21);
    assert_eq!(Genre::Short as u8, 22);
    assert_eq!(Genre::Sport as u8, 23);
    assert_eq!(Genre::TalkShow as u8, 24);
    assert_eq!(Genre::Thriller as u8, 25);
    assert_eq!(Genre::War as u8, 26);
    assert_eq!(Genre::Western as u8, 27);
    assert_eq!(Genre::Experimental as u8, 28);
  }

  #[test]
  fn test_genre_from_u8() {
    assert_eq!(Genre::Action, unsafe { Genre::from(0) });
    assert_eq!(Genre::Adult, unsafe { Genre::from(1) });
    assert_eq!(Genre::Adventure, unsafe { Genre::from(2) });
    assert_eq!(Genre::Animation, unsafe { Genre::from(3) });
    assert_eq!(Genre::Biography, unsafe { Genre::from(4) });
    assert_eq!(Genre::Comedy, unsafe { Genre::from(5) });
    assert_eq!(Genre::Crime, unsafe { Genre::from(6) });
    assert_eq!(Genre::Documentary, unsafe { Genre::from(7) });
    assert_eq!(Genre::Drama, unsafe { Genre::from(8) });
    assert_eq!(Genre::Family, unsafe { Genre::from(9) });
    assert_eq!(Genre::Fantasy, unsafe { Genre::from(10) });
    assert_eq!(Genre::FilmNoir, unsafe { Genre::from(11) });
    assert_eq!(Genre::GameShow, unsafe { Genre::from(12) });
    assert_eq!(Genre::History, unsafe { Genre::from(13) });
    assert_eq!(Genre::Horror, unsafe { Genre::from(14) });
    assert_eq!(Genre::Music, unsafe { Genre::from(15) });
    assert_eq!(Genre::Musical, unsafe { Genre::from(16) });
    assert_eq!(Genre::Mystery, unsafe { Genre::from(17) });
    assert_eq!(Genre::News, unsafe { Genre::from(18) });
    assert_eq!(Genre::RealityTv, unsafe { Genre::from(19) });
    assert_eq!(Genre::Romance, unsafe { Genre::from(20) });
    assert_eq!(Genre::SciFi, unsafe { Genre::from(21) });
    assert_eq!(Genre::Short, unsafe { Genre::from(22) });
    assert_eq!(Genre::Sport, unsafe { Genre::from(23) });
    assert_eq!(Genre::TalkShow, unsafe { Genre::from(24) });
    assert_eq!(Genre::Thriller, unsafe { Genre::from(25) });
    assert_eq!(Genre::War, unsafe { Genre::from(26) });
    assert_eq!(Genre::Western, unsafe { Genre::from(27) });
    assert_eq!(Genre::Experimental, unsafe { Genre::from(28) });
  }

  #[test]
  fn test_genre_max() {
    assert_eq!(Genre::max(), Genre::Experimental as u8);
  }

  #[test]
  #[should_panic]
  fn test_genres_max() {
    Genres::default().get(Genre::max() + 1);
  }

  fn make_genres() -> Genres {
    let mut genres = Genres::default();
    genres.add(Genre::Adventure);
    genres.add(Genre::Music);
    genres.add(Genre::War);
    genres
  }

  #[test]
  fn test_genres() {
    let genres = make_genres();
    assert_eq!(genres.get(2), Some(Genre::Adventure));
    assert_eq!(genres.get(3), None);
    assert_eq!(genres.get(15), Some(Genre::Music));
    assert_eq!(genres.get(17), None);
    assert_eq!(genres.get(26), Some(Genre::War));
    assert_eq!(genres.get(27), None);
  }

  #[test]
  fn test_genres_iter() {
    let genres = make_genres();
    let mut iter = genres.iter();
    assert_eq!(iter.next(), Some(Genre::Adventure));
    assert_eq!(iter.next(), Some(Genre::Music));
    assert_eq!(iter.next(), Some(Genre::War));
    assert_eq!(iter.next(), None);
  }
}
