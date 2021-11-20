#![warn(clippy::all)]

use super::error::Err;
use super::genre::{Genre, Genres};
use super::title::{Title, TitleId, TitleType};
use crate::mem::MemSize;
use crate::Res;
use atoi::atoi;
use derive_more::{Display, From};
use fnv::FnvHashMap;
use std::sync::Arc;
use std::{ops::Index, str::FromStr};

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From)]
struct MovieCookie(usize);

impl MemSize for MovieCookie {
  fn mem_size(&self) -> usize {
    self.0.mem_size()
  }
}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From)]
struct SeriesCookie(usize);

impl MemSize for SeriesCookie {
  fn mem_size(&self) -> usize {
    self.0.mem_size()
  }
}

type DbByYear<C> = FnvHashMap<Option<u16>, Vec<C>>;
type DbByName<C> = FnvHashMap<Arc<[u8]>, DbByYear<C>>;
type DbById<C> = FnvHashMap<TitleId, (C, Arc<[u8]>, Arc<[u8]>)>;

#[derive(Default)]
pub(crate) struct Basics {
  /// Movies information.
  movies: Vec<Title>,
  /// Map from movie names to years to movies.
  movies_db: DbByName<MovieCookie>,
  /// Map from IMDB ID to movie names.
  movies_ids_names: DbById<MovieCookie>,

  /// Series information.
  series: Vec<Title>,
  /// Map from series or episode names to years to series.
  series_db: DbByName<SeriesCookie>,
  /// Map from IMDB ID to series names.
  series_ids_names: DbById<SeriesCookie>,
}

impl MemSize for Basics {
  fn mem_size(&self) -> usize {
    self.movies.mem_size()
      + self.movies_db.mem_size()
      + self.movies_ids_names.mem_size()
      + self.series.mem_size()
      + self.series_db.mem_size()
      + self.series_ids_names.mem_size()
  }
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
  pub(crate) fn n_movies(&self) -> usize {
    self.movies.len()
  }

  pub(crate) fn n_series(&self) -> usize {
    self.series.len()
  }

  pub(crate) fn movie_names(&self, id: TitleId) -> Vec<&[u8]> {
    if let Some((_, ptitle, otitle)) = self.movies_ids_names.get(&id) {
      if Arc::ptr_eq(ptitle, otitle) {
        vec![ptitle]
      } else {
        vec![ptitle, otitle]
      }
    } else {
      vec![]
    }
  }

  pub(crate) fn movie_with_year(&self, name: &[u8], year: u16) -> Vec<&Title> {
    if let Some(by_year) = self.movies_db.get(name) {
      if let Some(cookies) = by_year.get(&Some(year)) {
        return cookies.iter().map(|cookie| &self[cookie]).collect();
      }
    }

    vec![]
  }

  pub(crate) fn movie(&self, name: &[u8]) -> Vec<&Title> {
    if let Some(by_year) = self.movies_db.get(name) {
      return by_year.values().flatten().map(|cookie| &self[cookie]).collect();
    }

    vec![]
  }

  pub(crate) fn series_names(&self, id: TitleId) -> Vec<&[u8]> {
    if let Some((_, ptitle, otitle)) = self.series_ids_names.get(&id) {
      if Arc::ptr_eq(ptitle, otitle) {
        vec![ptitle]
      } else {
        vec![ptitle, otitle]
      }
    } else {
      vec![]
    }
  }

  pub(crate) fn series_with_year(&self, name: &[u8], year: u16) -> Vec<&Title> {
    if let Some(by_year) = self.series_db.get(name) {
      if let Some(cookies) = by_year.get(&Some(year)) {
        return cookies.iter().map(|cookie| &self[cookie]).collect();
      }
    }

    vec![]
  }

  pub(crate) fn series(&self, name: &[u8]) -> Vec<&Title> {
    if let Some(by_year) = self.series_db.get(name) {
      return by_year.values().flatten().map(|cookie| &self[cookie]).collect();
    }

    vec![]
  }

  pub(crate) fn add_basics_from_line(&mut self, line: &[u8]) -> Res<()> {
    let mut iter = line.split(|&b| b == super::parsing::TAB);

    macro_rules! next {
      () => {{
        iter.next().ok_or(Err::Eof)?
      }};
    }

    let title_id = TitleId::from(super::parsing::parse_title_id(next!())?);

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

    let ptitle_lowercase = ptitle.to_ascii_lowercase();
    let (ptitle, otitle) = if ptitle.eq_ignore_ascii_case(otitle) {
      let ptitle = Arc::from(ptitle_lowercase);
      (Arc::clone(&ptitle), ptitle)
    } else {
      let otitle = otitle.to_ascii_lowercase();
      (Arc::from(ptitle_lowercase), Arc::from(otitle))
    };

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
          result.add_genre(genre);
        }
      }

      result
    };

    let title = Title::new(
      title_id,
      title_type,
      is_adult,
      start_year,
      end_year,
      runtime_minutes,
      genres,
    );

    if title_type.is_movie() {
      let cookie = MovieCookie::from(self.movies.len());
      self.movies.push(title);

      let value = (cookie, Arc::clone(&ptitle), Arc::clone(&otitle));
      if self.movies_ids_names.insert(title_id, value).is_some() {
        return Err::duplicate(title_id);
      }

      if !Arc::ptr_eq(&otitle, &ptitle) {
        Self::db(&mut self.movies_db, cookie, otitle, start_year);
      }

      Self::db(&mut self.movies_db, cookie, ptitle, start_year);
    } else if title_type.is_series() {
      let cookie = SeriesCookie::from(self.series.len());
      self.series.push(title);

      let value = (cookie, Arc::clone(&ptitle), Arc::clone(&otitle));
      if self.series_ids_names.insert(title_id, value).is_some() {
        return Err::duplicate(title_id);
      }

      if !Arc::ptr_eq(&otitle, &ptitle) {
        Self::db(&mut self.series_db, cookie, otitle, start_year);
      }

      Self::db(&mut self.series_db, cookie, ptitle, start_year);
    }

    Ok(())
  }

  fn db<T>(db: &mut DbByName<T>, cookie: T, title: Arc<[u8]>, year: Option<u16>)
  where
    T: From<usize> + Copy,
  {
    db.entry(title)
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
