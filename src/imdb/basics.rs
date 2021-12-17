#![warn(clippy::all)]

use super::error::Err;
use super::genre::{Genre, Genres};
use super::keywords::KeywordSet;
use super::title::{TitleId, TitleType};
use crate::imdb::title::TitleBasics;
use crate::Res;
use atoi::atoi;
use deepsize::DeepSizeOf;
use derive_more::{Display, From};
use fnv::FnvHashMap;
use std::ops::Index;
use std::str::FromStr;

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, DeepSizeOf)]
struct MoviesCookie(usize);

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, DeepSizeOf)]
struct SeriesCookie(usize);

type ById<C> = FnvHashMap<TitleId<'static>, C>;
type ByYear<C> = FnvHashMap<Option<u16>, Vec<C>>;
type ByTitle<C> = FnvHashMap<String, ByYear<C>>;

#[derive(Default, DeepSizeOf)]
pub(crate) struct Basics {
  /// Movies information.
  movies: Vec<TitleBasics>,
  /// Map from title ID to movie.
  movies_id: ById<MoviesCookie>,
  /// Map from movies names to years to movies.
  movies_titles: ByTitle<MoviesCookie>,

  /// Series information.
  series: Vec<TitleBasics>,
  /// Map from title ID to series.
  series_id: ById<SeriesCookie>,
  /// Map from series names to years to series.
  series_titles: ByTitle<SeriesCookie>,
}

impl Index<&MoviesCookie> for Basics {
  type Output = TitleBasics;

  fn index(&self, index: &MoviesCookie) -> &Self::Output {
    unsafe { self.movies.get_unchecked(index.0) }
  }
}

impl Index<&SeriesCookie> for Basics {
  type Output = TitleBasics;

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

  fn add_movie(&mut self, title: TitleBasics) -> MoviesCookie {
    let cookie = MoviesCookie::from(self.movies.len());
    self.movies.push(title);
    cookie
  }

  fn add_series(&mut self, title: TitleBasics) -> SeriesCookie {
    let cookie = SeriesCookie::from(self.series.len());
    self.series.push(title);
    cookie
  }

  pub(crate) fn movie_by_titleid(&self, title_id: &TitleId) -> Option<&TitleBasics> {
    self.movies_id.get(title_id).map(|cookie| &self[cookie])
  }

  pub(crate) fn movies_by_keyword(&self, keywords: KeywordSet) -> impl Iterator<Item = &TitleBasics> {
    self
      .movies_titles
      .iter()
      .filter(move |(title, _)| keywords.matches(title))
      .map(|(_, by_year)| by_year.values().flatten().map(|cookie| &self[cookie]))
      .flatten()
  }

  pub(crate) fn movies_by_title_year<'a>(
    &'a self,
    name: &str,
    year: u16,
  ) -> impl Iterator<Item = &TitleBasics> + 'a {
    let by_year = self.movies_titles.get(name);
    let cookies = by_year.map(|by_year| by_year.get(&Some(year)));
    let cookies = cookies.flatten();
    cookies
      .into_iter()
      .map(|cookies| cookies.iter())
      .flatten()
      .map(|cookie| &self[cookie])
  }

  pub(crate) fn movies_by_title<'a>(&'a self, name: &str) -> impl Iterator<Item = &TitleBasics> + 'a {
    let by_year = self.movies_titles.get(name);
    let cookies = by_year.map(|by_year| by_year.values());
    cookies.into_iter().flatten().flatten().map(|cookie| &self[cookie])
  }

  pub(crate) fn series_by_titleid(&self, title_id: &TitleId) -> Option<&TitleBasics> {
    self.series_id.get(title_id).map(|cookie| &self[cookie])
  }

  pub(crate) fn series_by_keyword(&self, keywords: KeywordSet) -> impl Iterator<Item = &TitleBasics> {
    self
      .series_titles
      .iter()
      .filter(move |(title, _)| keywords.matches(title))
      .map(|(_, by_year)| by_year.values().flatten().map(|cookie| &self[cookie]))
      .flatten()
  }

  pub(crate) fn series_by_title_year<'a>(
    &'a self,
    name: &str,
    year: u16,
  ) -> impl Iterator<Item = &TitleBasics> + 'a {
    let by_year = self.series_titles.get(name);
    let cookies = by_year.map(|by_year| by_year.get(&Some(year)));
    let cookies = cookies.flatten();
    cookies
      .into_iter()
      .map(|cookies| cookies.iter())
      .flatten()
      .map(|cookie| &self[cookie])
  }

  pub(crate) fn series_by_title<'a>(&'a self, name: &str) -> impl Iterator<Item = &TitleBasics> + 'a {
    let by_year = self.series_titles.get(name);
    let cookies = by_year.map(|by_year| by_year.values());
    cookies.into_iter().flatten().flatten().map(|cookie| &self[cookie])
  }

  pub(crate) fn add_basics_from_line(&mut self, line: &'static [u8]) -> Res<()> {
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
      return Ok(());
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
          result.add_genre(genre);
        }
      }

      result
    };

    let title = TitleBasics {
      title_id,
      title_type,
      primary_title,
      original_title,
      is_adult,
      start_year,
      end_year,
      runtime_minutes,
      genres,
    };

    if title_type.is_movie() {
      let cookie = self.add_movie(title);

      if self.movies_id.insert(title_id, cookie).is_some() {
        return Err::duplicate_id(title_id);
      }

      let lc_primary_title = primary_title.to_lowercase();
      let lc_original_title = original_title.to_lowercase();
      let same_title = lc_primary_title == lc_original_title;

      if let Some(escaped_primary_title) = Self::escape_title(&lc_primary_title) {
        Self::insert_title(&mut self.movies_titles, cookie, escaped_primary_title, start_year);
      }

      Self::insert_title(&mut self.movies_titles, cookie, lc_primary_title, start_year);

      if !same_title {
        if let Some(escaped_original_title) = Self::escape_title(&lc_original_title) {
          Self::insert_title(&mut self.movies_titles, cookie, escaped_original_title, start_year);
        }

        Self::insert_title(&mut self.movies_titles, cookie, lc_original_title, start_year);
      }
    } else if title_type.is_series() {
      let cookie = self.add_series(title);

      if self.series_id.insert(title_id, cookie).is_some() {
        return Err::duplicate_id(title_id);
      }

      let lc_primary_title = primary_title.to_lowercase();
      let lc_original_title = original_title.to_lowercase();
      let same_title = lc_primary_title == lc_original_title;

      if let Some(escaped_primary_title) = Self::escape_title(&lc_primary_title) {
        Self::insert_title(&mut self.series_titles, cookie, escaped_primary_title, start_year);
      }

      Self::insert_title(&mut self.series_titles, cookie, lc_primary_title, start_year);

      if !same_title {
        if let Some(escaped_original_title) = Self::escape_title(&lc_original_title) {
          Self::insert_title(&mut self.series_titles, cookie, escaped_original_title, start_year);
        }

        Self::insert_title(&mut self.series_titles, cookie, lc_original_title, start_year);
      }
    }

    Ok(())
  }

  fn escape_title(title: &str) -> Option<String> {
    const ES: &[char] = &['é', 'è', 'ê'];

    let mut needs_escape = false;
    for c in title.chars() {
      if ES.contains(&c) {
        needs_escape = true;
      }
    }

    if !needs_escape {
      return None;
    }

    let mut escaped = String::with_capacity(title.len());
    for c in title.chars() {
      if ES.contains(&c) {
        escaped.push('e');
      } else {
        escaped.push(c);
      }
    }

    Some(escaped)
  }

  fn insert_title<C>(db: &mut ByTitle<C>, cookie: C, title: String, year: Option<u16>)
  where
    C: From<usize> + Copy,
  {
    db.entry(title)
      .and_modify(|by_year| {
        by_year
          .entry(year)
          .and_modify(|titles| titles.push(cookie))
          .or_insert_with(|| vec![cookie]);
      })
      .or_insert_with(|| {
        let mut by_year = ByYear::default();
        by_year.insert(year, vec![cookie]);
        by_year
      });
  }
}
