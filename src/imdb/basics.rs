#![warn(clippy::all)]

use super::error::Err;
use super::parsing::LINES_PER_THREAD;
use super::title::{TitleBasics, TitleId};
use crate::Res;
use aho_corasick::AhoCorasickBuilder;
use derive_more::{Display, From, Into};
use deunicode::deunicode;
use fnv::{FnvHashMap, FnvHashSet};
use std::fmt;
use std::ops::Index;

#[derive(Clone, Copy)]
pub enum QueryType {
  Movies,
  Series,
}

impl fmt::Display for QueryType {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      QueryType::Movies => write!(f, "movie"),
      QueryType::Series => write!(f, "series"),
    }
  }
}

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, Into)]
struct MoviesCookie(usize);

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, Into)]
struct SeriesCookie(usize);

#[derive(Default)]
pub struct Basics {
  movies: BasicsImpl<MoviesCookie>,
  series: BasicsImpl<SeriesCookie>,
}

impl Basics {
  pub fn n_movies(&self) -> usize {
    self.movies.n_titles()
  }

  pub fn n_series(&self) -> usize {
    self.series.n_titles()
  }

  pub fn n_titles(&self) -> usize {
    self.n_movies() + self.n_series()
  }

  pub(crate) fn add(&mut self, line: &'static [u8]) -> Res<()> {
    let title = match TitleBasics::try_from(line) {
      Ok(title) => title,
      Err(err) => match *err.downcast::<Err>()? {
        Err::UnsupportedTitleType => return Ok(()),
        err => return Err(Box::new(err)),
      },
    };

    if title.title_type.is_movie() {
      self.movies.store_title(title);
    } else if title.title_type.is_series() {
      self.series.store_title(title);
    }

    Ok(())
  }

  pub(crate) fn by_id(&self, id: &TitleId, query_type: QueryType) -> Option<&TitleBasics> {
    match query_type {
      QueryType::Movies => self.movies.by_id(id),
      QueryType::Series => self.series.by_id(id),
    }
  }

  pub(crate) fn by_title<'a>(
    &'a self,
    title: &str,
    query_type: QueryType,
  ) -> Box<dyn Iterator<Item = &'a TitleBasics> + 'a> {
    match query_type {
      QueryType::Movies => self.movies.by_title(title),
      QueryType::Series => self.series.by_title(title),
    }
  }

  pub(crate) fn by_title_and_year<'a>(
    &'a self,
    title: &str,
    year: u16,
    query_type: QueryType,
  ) -> Box<dyn Iterator<Item = &'a TitleBasics> + 'a> {
    match query_type {
      QueryType::Movies => Box::new(self.movies.by_title_and_year(title, year)),
      QueryType::Series => Box::new(self.series.by_title_and_year(title, year)),
    }
  }

  pub(crate) fn by_keywords<'a>(
    &'a self,
    keywords: &'a [&'a str],
    query_type: QueryType,
  ) -> Box<dyn Iterator<Item = &'a TitleBasics> + 'a> {
    match query_type {
      QueryType::Movies => Box::new(self.movies.by_keywords(keywords)),
      QueryType::Series => Box::new(self.series.by_keywords(keywords)),
    }
  }
}

type ById<C> = FnvHashMap<usize, C>;
type ByYear<C> = FnvHashMap<u16, Vec<C>>;
type ByTitle<C> = FnvHashMap<String, ByYear<C>>;

struct BasicsImpl<C> {
  /// Titles information.
  titles: Vec<TitleBasics>,
  /// Map from title ID to Title.
  by_id: ById<C>,
  /// Map from years to title names to Titles.
  by_title: ByTitle<C>,
}

impl<C> Default for BasicsImpl<C> {
  fn default() -> Self {
    Self {
      titles: Vec::with_capacity(LINES_PER_THREAD),
      by_id: Default::default(),
      by_title: Default::default(),
    }
  }
}

impl<C: Into<usize>> Index<C> for BasicsImpl<C> {
  type Output = TitleBasics;

  fn index(&self, index: C) -> &Self::Output {
    unsafe { self.titles.get_unchecked(index.into()) }
  }
}

impl<C: From<usize>> BasicsImpl<C> {
  fn next_cookie(&self) -> C {
    C::from(self.n_titles())
  }
}

impl<C: From<usize> + Into<usize> + Copy> BasicsImpl<C> {
  fn store_title(&mut self, title: TitleBasics) {
    let cookie = self.next_cookie();

    self.insert_by_id(&title.title_id, cookie);

    let lc_primary_title = title.primary_title.to_lowercase();
    let lc_original_title = title.original_title.to_lowercase();
    let lc_same_title = lc_primary_title == lc_original_title;

    let deunicoded_primary_title = deunicode(&lc_primary_title);
    if deunicoded_primary_title != lc_primary_title {
      self.insert_by_title_and_year(deunicoded_primary_title, title.start_year, cookie);
    }

    self.insert_by_title_and_year(lc_primary_title, title.start_year, cookie);

    if !lc_same_title {
      let deunicoded_original_title = deunicode(&lc_original_title);
      if deunicoded_original_title != lc_original_title {
        self.insert_by_title_and_year(deunicoded_original_title, title.start_year, cookie);
      }

      self.insert_by_title_and_year(lc_original_title, title.start_year, cookie);
    }

    self.store(title);
  }
}

impl<C> BasicsImpl<C> {
  fn store(&mut self, title: TitleBasics) {
    self.titles.push(title);
  }

  fn n_titles(&self) -> usize {
    self.titles.len()
  }

  fn cookie_by_id(&self, id: &TitleId) -> Option<&C> {
    self.by_id.get(&id.as_usize())
  }

  fn cookies_by_title_and_year(&self, title: &str, year: u16) -> impl Iterator<Item = &C> {
    if let Some(by_year) = self.by_title.get(title) {
      if let Some(cookies) = by_year.get(&year) {
        return cookies.iter();
      }
    }

    [].iter()
  }

  fn cookies_by_keywords<'a>(&'a self, keywords: &'a [&'a str]) -> impl Iterator<Item = &'a C> {
    let searcher = AhoCorasickBuilder::new().build(keywords);
    self
      .by_title
      .iter()
      .filter(move |&(title, _)| {
        let matches: FnvHashSet<_> = searcher.find_iter(title).map(|mat| mat.pattern()).collect();
        matches.len() == keywords.len()
      })
      .map(|(_, by_year)| by_year.values())
      .flatten()
      .flatten()
  }

  fn insert_by_id(&mut self, id: &TitleId, cookie: C) -> bool {
    self.by_id.insert(id.as_usize(), cookie).is_none()
  }

  fn insert_by_title_and_year(&mut self, title: String, year: Option<u16>, cookie: C) {
    if let Some(year) = year {
      self.by_title.entry(title).or_default().entry(year).or_default().push(cookie);
    } else {
      self.by_title.entry(title).or_default().entry(0).or_default().push(cookie);
    }
  }
}

impl<C: Into<usize> + Copy> BasicsImpl<C> {
  pub(crate) fn by_id(&self, id: &TitleId) -> Option<&TitleBasics> {
    self.cookie_by_id(id).map(|&cookie| &self[cookie])
  }

  pub(crate) fn by_title<'a>(&'a self, title: &str) -> Box<dyn Iterator<Item = &TitleBasics> + 'a> {
    if let Some(by_year) = self.by_title.get(title) {
      return Box::new(by_year.values().flatten().map(|&cookie| &self[cookie]));
    }

    Box::new(std::iter::empty())
  }

  pub(crate) fn by_title_and_year(&self, title: &str, year: u16) -> impl Iterator<Item = &TitleBasics> {
    self.cookies_by_title_and_year(title, year).map(|&cookie| &self[cookie])
  }

  pub(crate) fn by_keywords<'a>(&'a self, keywords: &'a [&'a str]) -> impl Iterator<Item = &'a TitleBasics> {
    self.cookies_by_keywords(keywords).map(|&cookie| &self[cookie])
  }
}
