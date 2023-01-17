#![warn(clippy::all)]

use crate::imdb::error::Err;
use crate::imdb::genre::{Genre, Genres};
use crate::imdb::ratings::{Rating, Ratings};
use crate::imdb::title_header::TitleHeader;
use crate::imdb::title_id::TitleId;
use crate::imdb::title_type::TitleType;
use crate::imdb::tokens;
use crate::iter_next;
use crate::utils::result::Res;
use atoi::atoi;
use serde::Serialize;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::str::FromStr;
use std::time::Duration;

/// Wraps a title based on its type.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TsvAction<T> {
  Skip,
  Movie(T),
  Series(T),
}

impl<T> From<TsvAction<T>> for Option<T> {
  fn from(val: TsvAction<T>) -> Self {
    match val {
      TsvAction::Skip => None,
      TsvAction::Movie(t) => Some(t),
      TsvAction::Series(t) => Some(t),
    }
  }
}

/// Primary/original titles of a movie/series and its relevant information such as duration and rating
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Title<'storage> {
  #[serde(flatten)]
  header: TitleHeader,

  title_id: TitleId<'storage>,
  primary_title: &'storage str,
  original_title: Option<&'storage str>,
}

impl PartialEq for Title<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.title_id == other.title_id
  }
}

impl Eq for Title<'_> {}

impl Hash for Title<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.title_id.hash(state);
  }
}

impl<'storage> Title<'storage> {
  /// Returns the id of the title
  pub fn title_id(&self) -> &TitleId {
    &self.title_id
  }

  /// Returns the type of the title (e.g. Movie, Series etc.)
  pub fn title_type(&self) -> TitleType {
    self.header.title_type()
  }

  /// Returns the primary title in English
  pub fn primary_title(&self) -> &str {
    self.primary_title
  }

  /// Returns an Option containing the original title in the original language if exists
  pub fn original_title(&self) -> Option<&str> {
    self.original_title
  }

  /// Returns if the title is rated R
  pub fn is_adult(&self) -> bool {
    self.header.is_adult()
  }

  /// Returns the release date of the title
  pub fn start_year(&self) -> Option<u16> {
    self.header.start_year()
  }

  /// Returns the duration of the title
  pub fn runtime(&self) -> Option<Duration> {
    self
      .header
      .runtime_minutes()
      .map(|runtime| Duration::from_secs(u64::from(runtime) * 60))
  }

  /// Returns the set of genres associated with the title
  pub fn genres(&self) -> Genres {
    self.header.genres()
  }

  /// Returns the rating of the title
  pub fn rating(&self) -> Option<Rating> {
    self.header.rating()
  }

  /// Reads a title from tab separated values and returns it inside a TsvAction struct
  /// # Arguments
  /// * `line` - A title as tab separated values
  /// * `ratings` - Ratings struct containing the ratings of the titles
  pub(crate) fn from_tsv(line: &'storage [u8], ratings: &Ratings) -> Res<TsvAction<Self>> {
    let mut columns = line.split(|&b| b == tokens::TAB);

    let title_id = TitleId::try_from(iter_next!(columns))?;

    let title_type = {
      let title_type = iter_next!(columns);
      let title_type = unsafe { std::str::from_utf8_unchecked(title_type) };
      TitleType::from_str(title_type).map_err(|_| Err::TitleType)?
    };

    let is_movie = title_type.is_movie();
    let is_series = title_type.is_series();

    if !is_movie && !is_series {
      return Ok(TsvAction::Skip);
    }

    let primary_title = unsafe { std::str::from_utf8_unchecked(iter_next!(columns)) };
    let original_title = unsafe { std::str::from_utf8_unchecked(iter_next!(columns)) };
    let original_title = if original_title.to_lowercase() == primary_title.to_lowercase() {
      None
    } else {
      Some(original_title)
    };

    let is_adult = {
      let is_adult = iter_next!(columns);
      match is_adult {
        tokens::ZERO => false,
        tokens::ONE => true,
        _ => return Err::adult(),
      }
    };

    if is_adult {
      return Ok(TsvAction::Skip);
    }

    let start_year = {
      let start_year = iter_next!(columns);
      match start_year {
        tokens::NOT_AVAIL => None,
        start_year => Some(atoi::<u16>(start_year).ok_or(Err::StartYear)?),
      }
    };

    let _end_year = {
      let end_year = iter_next!(columns);
      match end_year {
        tokens::NOT_AVAIL => None,
        end_year => Some(atoi::<u16>(end_year).ok_or(Err::EndYear)?),
      }
    };

    let runtime_minutes = {
      let runtime_minutes = iter_next!(columns);
      match runtime_minutes {
        tokens::NOT_AVAIL => None,
        runtime_minutes => Some(atoi::<u16>(runtime_minutes).ok_or(Err::RuntimeMinutes)?),
      }
    };

    let genres = {
      let genres = iter_next!(columns);
      let mut result = Genres::default();

      if genres != tokens::NOT_AVAIL {
        let genres = genres.split(|&b| b == tokens::COMMA);
        for genre in genres {
          let genre = unsafe { std::str::from_utf8_unchecked(genre) };
          let genre = Genre::from_str(genre).map_err(|_| Err::Genre)?;
          result.add(genre);
        }
      }

      result
    };

    let rating = ratings.get(&title_id.as_usize()).copied();

    let header = TitleHeader::new_version_0(
      original_title.is_some(),
      is_adult,
      runtime_minutes,
      start_year,
      rating,
      title_type,
      genres,
    );

    let title = Title { header, title_id, primary_title, original_title };
    if is_movie {
      Ok(TsvAction::Movie(title))
    } else if is_series {
      Ok(TsvAction::Series(title))
    } else {
      Err::unsupported_title_type(title_type)
    }
  }

  /// Writes the title as binary
  /// # Arguments
  /// `writer` - Writer to write the title to
  pub(crate) fn write_binary<W: Write>(&self, writer: &mut W) -> Res {
    writer.write_all(&self.header.to_le_bytes())?;

    let title_id = self.title_id.as_bytes();
    writer.write_all(&(title_id.len() as u8).to_le_bytes())?;
    writer.write_all(title_id)?;

    let primary_title = self.primary_title.as_bytes();
    writer.write_all(&(primary_title.len() as u16).to_le_bytes())?;
    writer.write_all(primary_title)?;

    if let Some(original_title) = self.original_title {
      let original_title = original_title.as_bytes();
      writer.write_all(&(original_title.len() as u16).to_le_bytes())?;
      writer.write_all(original_title)?;
    }

    Ok(())
  }

  /// Reads a title from its binary representation and returns it inside a Result
  /// # Arguments
  /// * `source` - Title to be read as binary
  pub(crate) fn from_binary(source: &mut &'storage [u8]) -> Res<Self> {
    if (*source).len() < 23 {
      // # 23 bytes:
      //
      // * 16 bytes for header
      // * 1 byte for the title_id length
      // * At least 3 bytes for the title_id (ttX)
      // * 2 bytes for the primary title length
      // * At least 1 byte for the primary title

      return Err::eof();
    }

    let header: [u8; 16] = source[..16].try_into()?;
    let header = TitleHeader::from(header);

    *source = &source[16..];

    let title_id_len: [u8; 1] = source[..1].try_into()?;
    let title_id_len = u8::from_le_bytes(title_id_len) as usize;

    *source = &source[1..];

    let title_id = &source[..title_id_len];
    let title_id = TitleId::try_from(title_id)?;

    *source = &source[title_id_len..];

    let primary_title_len: [u8; 2] = source[..2].try_into()?;
    let primary_title_len = u16::from_le_bytes(primary_title_len) as usize;

    *source = &source[2..];

    let primary_title = &source[..primary_title_len];
    let primary_title = unsafe { std::str::from_utf8_unchecked(primary_title) };

    *source = &source[primary_title_len..];

    let original_title = if header.has_original_title() {
      let original_title_len: [u8; 2] = source[..2].try_into()?;
      let original_title_len = u16::from_le_bytes(original_title_len) as usize;

      *source = &source[2..];

      let original_title = &source[..original_title_len];
      let original_title = unsafe { std::str::from_utf8_unchecked(original_title) };

      *source = &source[original_title_len..];

      Some(original_title)
    } else {
      None
    };

    Ok(Self { header, title_id, primary_title, original_title })
  }
}

#[cfg(test)]
mod test_title {
  use crate::imdb::genre::Genre;
  use crate::imdb::ratings::Rating;
  use crate::imdb::ratings::Ratings;
  use crate::imdb::title::Title;
  use crate::imdb::title_type::TitleType;

  #[test]
  fn test_title() {
    let mut ratings = Ratings::default();
    ratings.insert(1, Rating::new(57, 1846));

    let title = Title::from_tsv(
      b"tt0000001\tshort\tCarmencita\tCarmencita\t0\t1894\t\\N\t1\tDocumentary,Short",
      &ratings,
    )
    .unwrap();
    let title: Option<Title> = title.into();
    let title = title.unwrap();

    assert_eq!(title.title_id().as_str(), "tt0000001");
    assert_eq!(title.title_type(), TitleType::Short);
    assert_eq!(title.primary_title(), "Carmencita");
    assert_eq!(title.original_title(), None);
    assert!(!title.is_adult());
    assert_eq!(title.start_year().unwrap(), 1894);
    assert_eq!(title.runtime().unwrap().as_secs(), 60);

    let mut genres_iter = title.genres().iter();
    assert_eq!(genres_iter.next().unwrap(), Genre::Documentary);
    assert_eq!(genres_iter.next().unwrap(), Genre::Short);

    assert_eq!(title.rating().unwrap().rating(), 57);
    assert_eq!(title.rating().unwrap().votes(), 1846);

    let mut binary = Vec::new();
    title.write_binary(&mut binary).unwrap();

    let title_parsed = Title::from_binary(&mut binary.as_ref()).unwrap();

    assert_eq!(title, title_parsed);
  }
}
