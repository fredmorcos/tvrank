#![warn(clippy::all)]

use crate::imdb::error::Err;
use crate::imdb::genre::{Genre, Genres};
use crate::imdb::ratings::{Rating, Ratings};
use crate::imdb::title_header::TitleHeader;
use crate::imdb::title_id::TitleId;
use crate::imdb::title_type::TitleType;
use crate::imdb::utils::tokens;
use crate::iter_next;
use crate::Res;
use atoi::atoi;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Title<'a> {
  header: TitleHeader,
  title_id: TitleId<'a>,
  primary_title: &'a str,
  original_title: Option<&'a str>,
}

impl Hash for Title<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.title_id.hash(state);
  }
}

impl<'a> Title<'a> {
  pub fn title_id(&self) -> &TitleId {
    &self.title_id
  }

  pub fn title_type(&self) -> TitleType {
    self.header.title_type()
  }

  pub fn primary_title(&self) -> &str {
    self.primary_title
  }

  pub fn original_title(&self) -> Option<&str> {
    self.original_title
  }

  pub fn is_adult(&self) -> bool {
    self.header.is_adult()
  }

  pub fn start_year(&self) -> Option<u16> {
    self.header.start_year()
  }

  pub fn runtime(&self) -> Option<Duration> {
    self
      .header
      .runtime_minutes()
      .map(|runtime| Duration::from_secs(u64::from(runtime) * 60))
  }

  pub fn genres(&self) -> Genres {
    self.header.genres()
  }

  pub fn rating(&self) -> Option<Rating> {
    self.header.rating()
  }

  pub(crate) fn from_tsv(line: &'a [u8], ratings: &Ratings) -> Res<Self> {
    let mut columns = line.split(|&b| b == tokens::TAB);

    let title_id = TitleId::try_from(iter_next!(columns))?;

    let title_type = {
      let title_type = iter_next!(columns);
      let title_type = unsafe { std::str::from_utf8_unchecked(title_type) };
      TitleType::from_str(title_type).map_err(|_| Err::TitleType)?
    };

    if !title_type.is_movie() && !title_type.is_series() {
      return Err(Box::new(Err::UnsupportedTitleType(title_type)));
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

    Ok(Title { header, title_id, primary_title, original_title })
  }

  pub(crate) fn write_binary<W: Write>(&self, writer: &mut W) -> Res<()> {
    let _ = writer.write_all(&self.header.to_le_bytes())?;

    let title_id = self.title_id.as_bytes();
    let _ = writer.write_all(&(title_id.len() as u8).to_le_bytes());
    let _ = writer.write_all(title_id)?;

    let primary_title = self.primary_title.as_bytes();
    let _ = writer.write_all(&(primary_title.len() as u16).to_le_bytes());
    let _ = writer.write_all(primary_title)?;

    if let Some(original_title) = self.original_title {
      let original_title = original_title.as_bytes();
      let _ = writer.write_all(&(original_title.len() as u16).to_le_bytes())?;
      let _ = writer.write_all(original_title)?;
    }

    Ok(())
  }

  pub(crate) fn from_binary(source: &mut &'a [u8]) -> Res<Self> {
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
  use super::Rating;
  use super::Ratings;
  use super::Title;

  #[test]
  fn test_title() {
    let mut ratings = Ratings::default();
    ratings.insert(1, Rating::new(57, 1846));

    let title = Title::from_tsv(
      b"tt0000001\tshort\tCarmencita\tCarmencita\t0\t1894\t\\N\t1\tDocumentary,Short",
      &ratings,
    )
    .unwrap();

    let mut binary = Vec::new();
    title.write_binary(&mut binary).unwrap();

    let title_parsed = Title::from_binary(&mut binary.as_ref()).unwrap();

    assert_eq!(title, title_parsed);
  }
}
