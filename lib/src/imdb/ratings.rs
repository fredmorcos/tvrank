#![warn(clippy::all)]

use crate::imdb::title_id::TitleId;
use crate::imdb::tokens;
use crate::iter_next;

use std::borrow::Borrow;
use std::cmp::{Ord, Ordering};
use std::io::{self, BufRead};
use std::num::ParseFloatError;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

use atoi::atoi;
use fnv::FnvHashMap;
use serde::Serialize;

/// Errors when converting titles from IMDB TSVs to binary.
#[derive(Debug, thiserror::Error)]
#[error("Error parsing title rating")]
pub enum Error {
  /// Title parsing error.
  #[error("Title parsing error: {0}")]
  TitleParsing(#[from] crate::imdb::title::Error),
  /// IO errors.
  #[error("IO error: {0}")]
  Io(#[from] io::Error),
  /// Number of votes is not a valid number.
  #[error("Number of votes is not a valid number")]
  Votes,
  /// ID already exists.
  #[error("Duplicate IMDB ID `{0}` found")]
  DuplicateId(String),
  /// Invalid rating value.
  #[error("Invalid rating value `{0}`")]
  InvalidRating(String, #[source] ParseFloatError),
  /// General parsing errors.
  #[error("Parsing error: {0}")]
  Parsing(#[from] crate::utils::tokens::Error),
  /// ID parsing errors.
  #[error("Error parsing ID: {0}")]
  IdParsing(#[from] crate::imdb::title_id::Error),
}

/// Average user rating of a title together with the number of votes
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
pub struct Rating {
  rating: u8,
  votes: u32,
}

impl PartialOrd for Rating {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Rating {
  fn cmp(&self, other: &Self) -> Ordering {
    match self.rating.cmp(&other.rating) {
      Ordering::Equal => {}
      ord => return ord,
    }

    self.votes.cmp(&other.votes)
  }
}

impl Rating {
  /// Create a new Rating
  /// # Arguments
  /// * `rating` - Average user rating
  /// * `votes` - Number of votes
  pub(crate) fn new(rating: u8, votes: u32) -> Self {
    Self { rating, votes }
  }

  /// Creates a Rating from a tab separated value
  /// * `columns` - Rating of a title as a tab separated value
  fn from_tsv<'a>(columns: &mut impl Iterator<Item = &'a [u8]>) -> Result<Self, Error> {
    let rating = unsafe { std::str::from_utf8_unchecked(iter_next!(columns)?) };
    let rating = f32::from_str(rating).map_err(|e| Error::InvalidRating(rating.to_owned(), e))?;
    let rating = unsafe { (rating * 10.0).to_int_unchecked() };
    let votes = atoi::<u32>(iter_next!(columns)?).ok_or(Error::Votes)?;
    Ok(Self::new(rating, votes))
  }

  /// Returns the average user rating
  pub fn rating(&self) -> u8 {
    self.rating
  }

  /// Returns the number of votes
  pub fn votes(&self) -> u32 {
    self.votes
  }
}

/// Maps a set of title IDs to their corresponding ratings
#[derive(Default)]
pub(crate) struct Ratings {
  ratings: FnvHashMap<usize, Rating>,
}

impl AsRef<FnvHashMap<usize, Rating>> for Ratings {
  fn as_ref(&self) -> &FnvHashMap<usize, Rating> {
    &self.ratings
  }
}

impl AsMut<FnvHashMap<usize, Rating>> for Ratings {
  fn as_mut(&mut self) -> &mut FnvHashMap<usize, Rating> {
    &mut self.ratings
  }
}

impl Borrow<FnvHashMap<usize, Rating>> for Ratings {
  fn borrow(&self) -> &FnvHashMap<usize, Rating> {
    &self.ratings
  }
}

impl Deref for Ratings {
  type Target = FnvHashMap<usize, Rating>;

  fn deref(&self) -> &Self::Target {
    &self.ratings
  }
}

impl DerefMut for Ratings {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.ratings
  }
}

impl Ratings {
  /// Create and return Ratings from tab separated values
  /// * `reader` - Reader containing a list of titles as tab separated values
  pub(crate) fn from_tsv<R: BufRead>(mut reader: R) -> Result<Self, Error> {
    // TODO: See if we can use byte vectors instead of strings. Ultimately we end up
    // parsing bytes rather than strings, and when we need strings, we already know they
    // are valid UTF-8 because we trust the source.

    let mut res = Self::default();
    let mut line = String::new();

    // Skip the first line.
    reader.read_line(&mut line)?;
    line.clear();

    loop {
      let bytes = reader.read_line(&mut line)?;

      if bytes == 0 {
        break;
      }

      let trimmed = line.trim_end();

      if trimmed.is_empty() {
        continue;
      }

      let mut columns = trimmed.as_bytes().split(|&b| b == tokens::TAB);

      let id = TitleId::try_from(iter_next!(columns)?)?;
      let rating = Rating::from_tsv(&mut columns)?;

      if res.insert(id.as_usize(), rating).is_some() {
        return Err(Error::DuplicateId(id.as_str().to_owned()));
      }

      line.clear();
    }

    Ok(res)
  }
}

#[cfg(test)]
mod tests_ratings {
  use crate::imdb::ratings::Rating;
  use crate::imdb::ratings::Ratings;
  use crate::imdb::title_id::TitleId;
  use indoc::indoc;
  use std::io::BufRead;

  fn make_ratings_reader() -> impl BufRead {
    indoc! {"
      tconst\taverageRating\tnumVotes
      tt0000001\t5.7\t1845
      tt0000002\t6.0\t236
      tt0000003\t6.5\t1603
      tt0000004\t6.0\t153
      tt0000005\t6.2\t2424
      tt0000006\t5.2\t158
      tt0000007\t5.4\t758
      tt0000008\t5.5\t1988
      tt0000009\t5.9\t191
      tt0000010\t6.9\t6636
    "}
    .as_bytes()
  }

  #[test]
  fn test_ratings_csv() {
    let reader = make_ratings_reader();
    let ratings = Ratings::from_tsv(reader).unwrap();
    assert_eq!(ratings.len(), 10);

    let id = TitleId::try_from("tt0000001".as_bytes()).unwrap();
    assert_eq!(ratings.get(&id.as_usize()), Some(&Rating::new(57, 1845)));

    let id = TitleId::try_from("tt0000010".as_bytes()).unwrap();
    assert_eq!(ratings.get(&id.as_usize()), Some(&Rating::new(69, 6636)));

    let id = TitleId::try_from("tt0000011".as_bytes()).unwrap();
    assert_eq!(ratings.get(&id.as_usize()), None);
  }
}
