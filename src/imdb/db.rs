#![warn(clippy::all)]

use crate::imdb::error::Err;
use crate::imdb::ratings::Ratings;
use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use crate::Res;
use aho_corasick::AhoCorasickBuilder;
use derive_more::{Display, From, Into};
use deunicode::deunicode;
use fnv::{FnvHashMap, FnvHashSet};
use std::fmt;
use std::io::{BufRead, Write};
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

pub struct Db {
  movies: DbImpl<MoviesCookie>,
  series: DbImpl<SeriesCookie>,
}

impl Db {
  pub fn with_capacities(movies_capacity: usize, series_capacity: usize) -> Self {
    Self { movies: DbImpl::with_capacity(movies_capacity), series: DbImpl::with_capacity(series_capacity) }
  }

  pub fn n_movies(&self) -> usize {
    self.movies.n_titles()
  }

  pub fn n_series(&self) -> usize {
    self.series.n_titles()
  }

  pub fn n_entries(&self) -> usize {
    self.n_movies() + self.n_series()
  }

  pub(crate) fn store_title(&mut self, title: Title<'static>) {
    let title_type = title.title_type();
    if title_type.is_movie() {
      self.movies.store_title(title);
    } else if title_type.is_series() {
      self.series.store_title(title);
    }
  }

  pub(crate) fn to_binary<R1: BufRead, R2: BufRead, W: Write>(
    ratings_reader: R1,
    mut basics_reader: R2,
    mut destination: W,
    progress_fn: &mut dyn FnMut(u64),
  ) -> Res<()> {
    let ratings = Ratings::from_tsv(ratings_reader, progress_fn)?;

    let mut line = String::new();

    // Skip the first line.
    let bytes = basics_reader.read_line(&mut line)?;
    progress_fn(bytes as u64);
    line.clear();

    loop {
      let bytes = basics_reader.read_line(&mut line)?;
      progress_fn(bytes as u64);

      if bytes == 0 {
        break;
      }

      let trimmed = line.trim_end();

      if trimmed.is_empty() {
        continue;
      }

      match Title::from_tsv(trimmed.as_bytes(), &ratings) {
        Ok(title) => title.write_binary(&mut destination)?,
        Err(e) => match e.downcast_ref::<Err>() {
          Some(downcast_e) => {
            if matches!(*downcast_e, Err::UnsupportedTitleType(_)) {
              line.clear();
              continue;
            } else {
              return Err(e);
            }
          }
          None => return Err(e),
        },
      }

      line.clear();
    }

    Ok(())
  }

  pub(crate) fn by_id(&self, id: &TitleId, query_type: QueryType) -> Option<&Title> {
    match query_type {
      QueryType::Movies => self.movies.by_id(id),
      QueryType::Series => self.series.by_id(id),
    }
  }

  pub(crate) fn by_title<'a>(
    &'a self,
    title: &str,
    query_type: QueryType,
  ) -> Box<dyn Iterator<Item = &'a Title> + 'a> {
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
  ) -> Box<dyn Iterator<Item = &'a Title> + 'a> {
    match query_type {
      QueryType::Movies => Box::new(self.movies.by_title_and_year(title, year)),
      QueryType::Series => Box::new(self.series.by_title_and_year(title, year)),
    }
  }

  pub(crate) fn by_keywords<'a, 'k: 'a>(
    &'a self,
    keywords: &'k [&str],
    query_type: QueryType,
  ) -> Box<dyn Iterator<Item = &'a Title> + 'a> {
    match query_type {
      QueryType::Movies => Box::new(self.movies.by_keywords(keywords)),
      QueryType::Series => Box::new(self.series.by_keywords(keywords)),
    }
  }
}

type ById<C> = FnvHashMap<usize, C>;
type ByYear<C> = FnvHashMap<u16, Vec<C>>;
type ByTitle<C> = FnvHashMap<String, ByYear<C>>;

struct DbImpl<C> {
  /// Titles information.
  titles: Vec<Title<'static>>,
  /// Map from title ID to Title.
  by_id: ById<C>,
  /// Map from years to title names to Titles.
  by_title: ByTitle<C>,
}

impl<C: Into<usize>> Index<C> for DbImpl<C> {
  type Output = Title<'static>;

  fn index(&self, index: C) -> &Self::Output {
    unsafe { self.titles.get_unchecked(index.into()) }
  }
}

impl<C: From<usize>> DbImpl<C> {
  fn next_cookie(&self) -> C {
    C::from(self.n_titles())
  }
}

impl<C: From<usize> + Into<usize> + Copy> DbImpl<C> {
  fn store_title(&mut self, title: Title<'static>) {
    let cookie = self.next_cookie();

    self.insert_by_id(title.title_id(), cookie);

    let lc_primary_title = title.primary_title().to_lowercase();

    let deunicoded_primary_title = deunicode(&lc_primary_title);
    if deunicoded_primary_title != lc_primary_title {
      self.insert_by_title_and_year(deunicoded_primary_title, title.start_year(), cookie);
    }

    self.insert_by_title_and_year(lc_primary_title, title.start_year(), cookie);

    if let Some(original_title) = title.original_title() {
      let lc_original_title = original_title.to_lowercase();

      let deunicoded_original_title = deunicode(&lc_original_title);
      if deunicoded_original_title != lc_original_title {
        self.insert_by_title_and_year(deunicoded_original_title, title.start_year(), cookie);
      }

      self.insert_by_title_and_year(lc_original_title, title.start_year(), cookie);
    }

    self.store(title);
  }
}

impl<C> DbImpl<C> {
  fn with_capacity(capacity: usize) -> Self {
    Self { titles: Vec::with_capacity(capacity), by_id: Default::default(), by_title: Default::default() }
  }

  fn store(&mut self, title: Title<'static>) {
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

  fn cookies_by_keywords<'a, 'k: 'a>(&'a self, keywords: &'k [&str]) -> impl Iterator<Item = &'a C> {
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

impl<C: Into<usize> + Copy> DbImpl<C> {
  pub(crate) fn by_id(&self, id: &TitleId) -> Option<&Title> {
    self.cookie_by_id(id).map(|&cookie| &self[cookie])
  }

  pub(crate) fn by_title<'a>(&'a self, title: &str) -> Box<dyn Iterator<Item = &Title> + 'a> {
    if let Some(by_year) = self.by_title.get(title) {
      return Box::new(by_year.values().flatten().map(|&cookie| &self[cookie]));
    }

    Box::new(std::iter::empty())
  }

  pub(crate) fn by_title_and_year(&self, title: &str, year: u16) -> impl Iterator<Item = &Title> {
    self.cookies_by_title_and_year(title, year).map(|&cookie| &self[cookie])
  }

  pub(crate) fn by_keywords<'a, 'k: 'a>(&'a self, keywords: &'k [&str]) -> impl Iterator<Item = &'a Title> {
    self.cookies_by_keywords(keywords).map(|&cookie| &self[cookie])
  }
}

#[cfg(test)]
mod test_db {
  use super::Db;
  use super::Ratings;
  use super::Title;
  use indoc::indoc;
  use std::io::BufRead;
  use std::io::Read;

  fn make_basics_reader() -> impl BufRead {
    indoc! {"
      tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres
      tt0000001\tshort\tCarmencita\tCarmencita\t0\t1894\t\\N\t1\tDocumentary,Short
      tt0000002\tshort\tLe clown et ses chiens\tLe clown et ses chiens\t0\t1892\t\\N\t5\tAnimation,Short
      tt0000003\tshort\tPauvre Pierrot\tPauvre Pierrot\t0\t1892\t\\N\t4\tAnimation,Comedy,Romance
      tt0000004\tshort\tUn bon bock\tUn bon bock\t0\t1892\t\\N\t12\tAnimation,Short
      tt0000005\tshort\tBlacksmith Scene\tBlacksmith Scene\t0\t1893\t\\N\t1\tComedy,Short
      tt0000006\tshort\tChinese Opium Den\tChinese Opium Den\t0\t1894\t\\N\t1\tShort
      tt0000007\tshort\tCorbett and Courtney Before the Kinetograph\tCorbett and Courtney Before the Kinetograph\t0\t1894\t\\N\t1\tShort,Sport
      tt0000008\tshort\tEdison Kinetoscopic Record of a Sneeze\tEdison Kinetoscopic Record of a Sneeze\t0\t1894\t\\N\t1\tDocumentary,Short
      tt0000009\tshort\tMiss Jerry\tMiss Jerry\t0\t1894\t\\N\t40\tRomance,Short
      tt0000010\tshort\tLeaving the Factory\tLa sortie de l'usine Lumière à Lyon\t0\t1895\t\\N\t1\tDocumentary,Short
    "}.as_bytes()
  }

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
  fn test_to_binary() {
    let basics_reader = make_basics_reader();
    let ratings_reader = make_ratings_reader();

    let mut storage = Vec::new();
    Db::to_binary(ratings_reader, basics_reader, &mut storage, &mut |_| {}).unwrap();

    let mut basics_reader = make_basics_reader();
    let ratings_reader = make_ratings_reader();

    let ratings = Ratings::from_tsv(ratings_reader, &mut |_| {}).unwrap();

    let mut basics_data = String::new();
    basics_reader.read_to_string(&mut basics_data).unwrap();

    let mut titles_from_tsv = Vec::new();

    let mut tsv_lines_iter = basics_data.lines();

    // Ignore first line.
    tsv_lines_iter.next();

    for line in tsv_lines_iter {
      let title = Title::from_tsv(line.as_bytes(), &ratings).unwrap();
      titles_from_tsv.push(title);
    }

    let mut titles_from_binary = Vec::new();
    let cursor: &mut &[u8] = &mut storage.as_ref();
    loop {
      if (*cursor).is_empty() {
        break;
      }

      let title = Title::from_binary(cursor).unwrap();
      titles_from_binary.push(title);
    }

    assert_eq!(titles_from_tsv, titles_from_binary);
  }
}
