#![warn(clippy::all)]

use super::{
  error::Err,
  genre::{Genre, Genres},
  title::{Title, TitleType},
};
use crate::Res;
use atoi::atoi;
use derive_more::{Display, From};
use fnv::FnvHashMap;
use std::{ops::Index, str::FromStr};

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From)]
struct TitleId(u64);

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From)]
struct MovieCookie(usize);

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From)]
struct SeriesCookie(usize);

type DbByYear<C> = FnvHashMap<Option<u16>, Vec<C>>;
type DbByName<C> = FnvHashMap<String, DbByYear<C>>;
type DbById<C> = FnvHashMap<TitleId, C>;

#[derive(Default)]
pub(crate) struct Basics {
  /// Movies information.
  movies: Vec<Title>,
  /// Map from movie names to years to movies.
  movies_db: DbByName<MovieCookie>,
  /// Map from IMDB ID to movies.
  movies_ids: DbById<MovieCookie>,

  /// Series information.
  series: Vec<Title>,
  /// Map from series or episode names to years to series.
  series_db: DbByName<SeriesCookie>,
  /// Map from IMDB ID to series.
  series_ids: DbById<SeriesCookie>,
}

impl Index<&MovieCookie> for Basics {
  type Output = Title;

  fn index(&self, index: &MovieCookie) -> &Self::Output {
    unsafe { self.movies.get_unchecked(index.0) }
  }
}

impl Index<&SeriesCookie> for Basics {
  type Output = Title;

  fn index(&self, index: &SeriesCookie) -> &Self::Output {
    unsafe { self.series.get_unchecked(index.0) }
  }
}

impl Basics {
  pub(crate) fn movie_with_year(&self, name: &str, year: u16) -> Vec<&Title> {
    if let Some(by_year) = self.movies_db.get(name) {
      if let Some(cookies) = by_year.get(&Some(year)) {
        return cookies.iter().map(|cookie| &self[cookie]).collect();
      }
    }

    vec![]
  }

  pub(crate) fn movie(&self, name: &str) -> Vec<&Title> {
    if let Some(by_year) = self.movies_db.get(name) {
      return by_year.values().flatten().map(|cookie| &self[cookie]).collect();
    }

    vec![]
  }

  pub(crate) fn series_with_year(&self, name: &str, year: u16) -> Vec<&Title> {
    if let Some(by_year) = self.series_db.get(name) {
      if let Some(cookies) = by_year.get(&Some(year)) {
        return cookies.iter().map(|cookie| &self[cookie]).collect();
      }
    }

    vec![]
  }

  pub(crate) fn series(&self, name: &str) -> Vec<&Title> {
    if let Some(by_year) = self.series_db.get(name) {
      return by_year.values().flatten().map(|cookie| &self[cookie]).collect();
    }

    vec![]
  }

  pub(crate) fn add_from_line(&mut self, line: &[u8]) -> Res<()> {
    const TAB: u8 = b'\t';
    const COMMA: u8 = b',';

    const TT: &[u8] = b"tt";
    const ZERO: &[u8] = b"0";
    const ONE: &[u8] = b"1";

    const NOT_AVAIL: &[u8] = b"\\N";

    let mut iter = line.split(|&b| b == TAB);

    macro_rules! next {
      () => {{
        iter.next().ok_or(Err::Eof)?
      }};
    }

    let id = {
      let id = next!();

      if &id[0..=1] != TT {
        return Err::id();
      }

      atoi::<u64>(&id[2..]).ok_or(Err::IdNumber)?
    };

    let title_type = {
      let title_type = next!();
      let title_type = unsafe { std::str::from_utf8_unchecked(title_type) };
      TitleType::from_str(title_type).map_err(|_| Err::TitleType)?
    };

    if !title_type.is_movie() && !title_type.is_series() {
      return Ok(());
    }

    let ptitle = next!();
    let otitle = next!();

    let is_adult = {
      let is_adult = next!();
      match is_adult {
        ZERO => false,
        ONE => true,
        _ => return Err::adult(),
      }
    };

    let start_year = {
      let start_year = next!();
      match start_year {
        NOT_AVAIL => None,
        start_year => Some(atoi::<u16>(start_year).ok_or(Err::StartYear)?),
      }
    };

    let end_year = {
      let end_year = next!();
      match end_year {
        NOT_AVAIL => None,
        end_year => Some(atoi::<u16>(end_year).ok_or(Err::EndYear)?),
      }
    };

    let runtime_minutes = {
      let runtime_minutes = next!();
      match runtime_minutes {
        NOT_AVAIL => None,
        runtime_minutes => Some(atoi::<u16>(runtime_minutes).ok_or(Err::RuntimeMinutes)?),
      }
    };

    let genres = {
      let genres = next!();
      let mut result = Genres::default();

      if genres != NOT_AVAIL {
        let genres = genres.split(|&b| b == COMMA);
        for genre in genres {
          let genre = unsafe { std::str::from_utf8_unchecked(genre) };
          let genre = Genre::from_str(genre).map_err(|_| Err::Genre)?;
          result.add_genre(genre);
        }
      }

      result
    };

    let title_id = TitleId(id);
    let title =
      Title::new(title_type, is_adult, start_year, end_year, runtime_minutes, genres);

    if title_type.is_movie() {
      let cookie = MovieCookie::from(self.movies.len());
      self.movies.push(title);

      if self.movies_ids.insert(title_id, cookie).is_some() {
        return Err::duplicate(title_id.0);
      }

      Self::db(&mut self.movies_db, cookie, ptitle, start_year);

      if otitle != ptitle {
        Self::db(&mut self.movies_db, cookie, otitle, start_year);
      }
    } else if title_type.is_series() {
      let cookie = SeriesCookie::from(self.series.len());
      self.series.push(title);

      if self.series_ids.insert(title_id, cookie).is_some() {
        return Err::duplicate(title_id.0);
      }

      Self::db(&mut self.series_db, cookie, ptitle, start_year);

      if otitle != ptitle {
        Self::db(&mut self.series_db, cookie, otitle, start_year);
      }
    }

    Ok(())
  }

  fn db<T>(db: &mut DbByName<T>, cookie: T, name: &[u8], year: Option<u16>)
  where
    T: From<usize> + Copy,
  {
    let name = unsafe { std::str::from_utf8_unchecked(name) };
    let name = name.to_ascii_lowercase();
    db.entry(name)
      .and_modify(|by_year| {
        by_year
          .entry(year)
          .and_modify(|titles| titles.push(cookie))
          .or_insert_with(|| vec![cookie]);
      })
      .or_insert_with(|| {
        let mut by_year = DbByYear::default();
        by_year.insert(year, vec![cookie]);
        by_year
      });
  }
}
