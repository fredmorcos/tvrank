#![warn(clippy::all)]

use super::error::Err;
use super::genre::{Genre, Genres};
use super::title::{TitleBasics, TitleId, TitleType};
use crate::trie::Trie;
use crate::Res;
use atoi::atoi;
use deepsize::DeepSizeOf;
use derive_more::{Display, From};
use deunicode::deunicode;
use nohash::IntMap;
use std::ops::Index;
use std::str::FromStr;

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, DeepSizeOf)]
struct MoviesCookie(usize);

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, DeepSizeOf)]
struct SeriesCookie(usize);

type ById<C> = Trie<C>;
type ByYear<C> = IntMap<u16, Vec<C>>;
type ByTitle<C> = Trie<ByYear<C>>;

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
    self.movies_id.lookup_exact(title_id.as_str()).map(|cookie| &self[cookie])
  }

  pub(crate) fn movies_by_keyword<'a>(
    &'a self,
    keywords: &'a [&str],
  ) -> impl Iterator<Item = &'a TitleBasics> {
    keywords
      .iter()
      .map(|&keyword| self.movies_titles.lookup_keyword(keyword))
      .map(|by_year| by_year.map(|by_year| by_year.values()))
      .flatten()
      .flatten()
      .flatten()
      .map(|cookie| &self[cookie])
  }

  pub(crate) fn movies_by_title_year<'a>(
    &'a self,
    name: &str,
    year: u16,
  ) -> impl Iterator<Item = &'a TitleBasics> {
    let by_year = self.movies_titles.lookup_exact(name);
    let cookies = by_year.map(|by_year| by_year.get(&year));
    let cookies = cookies.flatten();
    cookies
      .into_iter()
      .map(|cookies| cookies.iter())
      .flatten()
      .map(|cookie| &self[cookie])
  }

  pub(crate) fn movies_by_title<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a TitleBasics> {
    let by_year = self.movies_titles.lookup_exact(name);
    let cookies = by_year.map(|by_year| by_year.values());
    cookies.into_iter().flatten().flatten().map(|cookie| &self[cookie])
  }

  pub(crate) fn series_by_titleid(&self, title_id: &TitleId) -> Option<&TitleBasics> {
    self.series_id.lookup_exact(title_id.as_str()).map(|cookie| &self[cookie])
  }

  pub(crate) fn series_by_keyword<'a>(
    &'a self,
    keywords: &'a [&str],
  ) -> impl Iterator<Item = &'a TitleBasics> {
    keywords
      .iter()
      .map(|keyword| self.series_titles.lookup_keyword(keyword))
      .map(|by_year| by_year.map(|by_year| by_year.values()))
      .flatten()
      .flatten()
      .flatten()
      .map(|cookie| &self[cookie])
  }

  pub(crate) fn series_by_title_year<'a>(
    &'a self,
    name: &str,
    year: u16,
  ) -> impl Iterator<Item = &'a TitleBasics> {
    let by_year = self.series_titles.lookup_exact(name);
    let cookies = by_year.map(|by_year| by_year.get(&year));
    let cookies = cookies.flatten();
    cookies
      .into_iter()
      .map(|cookies| cookies.iter())
      .flatten()
      .map(|cookie| &self[cookie])
  }

  pub(crate) fn series_by_title<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a TitleBasics> {
    let by_year = self.series_titles.lookup_exact(name);
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

    let (start_year, db_start_year) = {
      let start_year = next!();
      match start_year {
        super::parsing::NOT_AVAIL => (None, 0),
        start_year => {
          let start_year = atoi::<u16>(start_year).ok_or(Err::StartYear)?;
          (Some(start_year), start_year)
        }
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
      self.movies_id.insert(title_id.as_str(), cookie);

      let lc_primary_title = primary_title.to_lowercase();
      let lc_original_title = original_title.to_lowercase();
      let lc_same_title = lc_primary_title == lc_original_title;

      let deunicoded_primary_title = deunicode(&lc_primary_title);
      if deunicoded_primary_title != lc_primary_title {
        Self::insert_title(&mut self.movies_titles, cookie, &deunicoded_primary_title, db_start_year);
      }

      Self::insert_title(&mut self.movies_titles, cookie, &lc_primary_title, db_start_year);

      if !lc_same_title {
        let deunicoded_original_title = deunicode(&lc_original_title);
        if deunicoded_original_title != lc_original_title {
          Self::insert_title(&mut self.movies_titles, cookie, &deunicoded_original_title, db_start_year);
        }

        Self::insert_title(&mut self.movies_titles, cookie, &lc_original_title, db_start_year);
      }
    } else if title_type.is_series() {
      let cookie = self.add_series(title);
      self.series_id.insert(title_id.as_str(), cookie);

      let lc_primary_title = primary_title.to_lowercase();
      let lc_original_title = original_title.to_lowercase();
      let lc_same_title = lc_primary_title == lc_original_title;

      let deunicoded_primary_title = deunicode(&lc_primary_title);
      if deunicoded_primary_title != lc_primary_title {
        Self::insert_title(&mut self.series_titles, cookie, &deunicoded_primary_title, db_start_year);
      }

      Self::insert_title(&mut self.series_titles, cookie, &lc_primary_title, db_start_year);

      if !lc_same_title {
        let deunicoded_original_title = deunicode(&lc_original_title);
        if deunicoded_original_title != lc_original_title {
          Self::insert_title(&mut self.series_titles, cookie, &deunicoded_original_title, db_start_year);
        }

        Self::insert_title(&mut self.series_titles, cookie, &lc_original_title, db_start_year);
      }
    }

    Ok(())
  }

  fn insert_title<C>(db: &mut ByTitle<C>, cookie: C, title: &str, year: u16)
  where
    C: From<usize> + Copy,
  {
    db.insert(title, ByYear::default())
      .entry(year)
      .or_insert_with(Vec::new)
      .push(cookie)
  }
}
