#![warn(clippy::all)]

use crate::ratings::Ratings;
use crate::title::Title;
use crate::title::TsvAction;
use crate::title_id::TitleId;
use aho_corasick::AhoCorasickBuilder;
use aho_corasick::MatchKind as ACMatchKind;
use derive_more::{Display, From, Into};
use deunicode::deunicode;
use fnv::{FnvHashMap, FnvHashSet};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::ops::Index;
use tvrank_utils::result::Res;

/// Specifies the type of title a query is for. E.g. Movies or Series.
#[derive(Clone, Copy, Display)]
#[display(fmt = "{}")]
pub enum Query {
  /// Query the database of Movies.
  #[display(fmt = "movie")]
  Movies,

  /// Query the database of Series.
  #[display(fmt = "series")]
  Series,
}

/// A special object (i.e. a handle) that is used to refer to a movie in the database.
#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, Into)]
struct MoviesCookie(usize);

/// A special object (i.e. a handle) that is used to refer to a series in the database.
#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy, From, Into)]
struct SeriesCookie(usize);

/// The primary API for access the movies and series database.
pub struct Db {
  movies: DbImpl<MoviesCookie>,
  series: DbImpl<SeriesCookie>,
}

impl Db {
  /// Construct a database for movies and series, with a starting capacity for each.
  ///
  /// # Arguments
  ///
  /// * `movies_cap` - Starting capacity of the movies database.
  /// * `series_cap` - Starting capacity of the series database.
  pub fn with_capacities(movies_cap: usize, series_cap: usize) -> Self {
    let movies = DbImpl::with_capacity(movies_cap);
    let series = DbImpl::with_capacity(series_cap);
    Self { movies, series }
  }

  /// The number of titles in the movies database.
  pub fn n_movies(&self) -> usize {
    self.movies.n_titles()
  }

  /// The number of titles in the series database.
  pub fn n_series(&self) -> usize {
    self.series.n_titles()
  }

  /// The total number of titles in the database.
  pub fn n_entries(&self) -> usize {
    self.n_movies() + self.n_series()
  }

  /// Insert a given title into the movies database.
  ///
  /// # Arguments
  ///
  /// * `title` - The title to be inserted.
  pub(crate) fn store_movie(&mut self, title: Title<'static>) {
    self.movies.store_title(title)
  }

  /// Insert the given title into the series database.
  ///
  /// # Arguments
  ///
  /// * `title` - The title to be inserted.
  pub(crate) fn store_series(&mut self, title: Title<'static>) {
    self.series.store_title(title)
  }

  /// Convert title data from tab separated values (TSVs) to binary.
  ///
  /// # Arguments
  ///
  /// * `ratings_reader` - TSV reader for ratings.
  /// * `basics_reader` - TSV reader for title data.
  /// * `movies_db_writer` - Binary writer to store movies.
  /// * `series_db_writer` - Binary writer to store series.
  pub(crate) fn to_binary<R1: BufRead, R2: BufRead, W1: Write, W2: Write>(
    ratings_reader: R1,
    mut basics_reader: R2,
    mut movies_db_writer: W1,
    mut series_db_writer: W2,
  ) -> Res<()> {
    let ratings = Ratings::from_tsv(ratings_reader)?;

    let mut line = String::new();

    // Skip the first line.
    basics_reader.read_line(&mut line)?;
    line.clear();

    loop {
      let bytes = basics_reader.read_line(&mut line)?;

      if bytes == 0 {
        break;
      }

      let trimmed = line.trim_end();

      if trimmed.is_empty() {
        continue;
      }

      match Title::from_tsv(trimmed.as_bytes(), &ratings)? {
        TsvAction::Movie(title) => title.write_binary(&mut movies_db_writer)?,
        TsvAction::Series(title) => title.write_binary(&mut series_db_writer)?,
        TsvAction::Skip => {
          line.clear();
          continue;
        }
      }

      line.clear();
    }

    Ok(())
  }

  /// Return the title with the given ID from the database.
  ///
  /// # Arguments
  ///
  /// * `id` - Title ID to lookup.
  /// * `query` - Whether to query movies or series.
  pub(crate) fn by_id(&self, id: &TitleId, query: Query) -> Option<&Title> {
    match query {
      Query::Movies => self.movies.by_id(id),
      Query::Series => self.series.by_id(id),
    }
  }

  /// Search for titles by name.
  ///
  /// # Arguments
  ///
  /// * `title` - The title name to search for.
  /// * `query` - Whether to query movies or series.
  pub(crate) fn by_title<'a>(
    &'a self,
    title: &str,
    query: Query,
  ) -> Box<dyn Iterator<Item = &'a Title> + 'a> {
    match query {
      Query::Movies => self.movies.by_title(title),
      Query::Series => self.series.by_title(title),
    }
  }

  /// Search for titles by name and year.
  ///
  /// # Arguments
  ///
  /// * `title` - The title name to search for.
  /// * `year` - The year to search for titles in.
  /// * `query` - Whether to query movies or series.
  pub(crate) fn by_title_and_year<'a>(
    &'a self,
    title: &str,
    year: u16,
    query: Query,
  ) -> Box<dyn Iterator<Item = &'a Title> + 'a> {
    match query {
      Query::Movies => Box::new(self.movies.by_title_and_year(title, year)),
      Query::Series => Box::new(self.series.by_title_and_year(title, year)),
    }
  }

  /// Search for titles by keywords.
  ///
  /// # Arguments
  ///
  /// * `keywords` - Keywords to search for in title names.
  /// * `query` - Whether to query movies or series.
  pub(crate) fn by_keywords<'a, 'k>(
    &'a self,
    keywords: &'k [&str],
    query: Query,
  ) -> Box<dyn Iterator<Item = &'a Title> + 'a> {
    match query {
      Query::Movies => Box::new(self.movies.by_keywords(keywords)),
      Query::Series => Box::new(self.series.by_keywords(keywords)),
    }
  }

  /// Search for titles by keywords and year.
  ///
  /// # Arguments
  ///
  /// * `keywords` - Keywords to search for in title names.
  /// * `year` - The year to search for titles in.
  /// * `query` - Whether to query movies or series.
  pub(crate) fn by_keywords_and_year<'a, 'k>(
    &'a self,
    keywords: &'k [&str],
    year: u16,
    query: Query,
  ) -> Box<dyn Iterator<Item = &'a Title> + 'a> {
    match query {
      Query::Movies => Box::new(self.movies.by_keywords_and_year(keywords, year)),
      Query::Series => Box::new(self.series.by_keywords_and_year(keywords, year)),
    }
  }
}

type ById<C> = FnvHashMap<usize, C>;
type ByYear<C> = FnvHashMap<u16, Vec<C>>;
type ByTitle<C> = HashMap<String, ByYear<C>>;

struct DbImpl<C> {
  /// The actual storage of title information.
  titles: Vec<Title<'static>>,
  /// Map from title IDs to Titles.
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
  /// Insert a given title into the database.
  ///
  /// # Arguments
  ///
  /// * `title` - The title to be inserted.
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
  /// Construct a database with a starting capacity.
  ///
  /// # Arguments
  ///
  /// * `cap` - Starting capacity of the database.
  fn with_capacity(cap: usize) -> Self {
    let titles = Vec::with_capacity(cap);
    let by_id = Default::default();
    let by_title = Default::default();
    Self { titles, by_id, by_title }
  }

  /// Insert a title into the database.
  ///
  /// # Arguments
  ///
  /// * `title` - The title to be stored.
  fn store(&mut self, title: Title<'static>) {
    self.titles.push(title);
  }

  /// The number of titles stored in the database.
  fn n_titles(&self) -> usize {
    self.titles.len()
  }

  /// Return a cookie for the given title ID.
  ///
  /// # Arguments
  ///
  /// * `id` - Title ID to search for.
  fn cookie_by_id(&self, id: &TitleId) -> Option<&C> {
    self.by_id.get(&id.as_usize())
  }

  /// Search for titles with the given title and year.
  ///
  /// # Arguments
  ///
  /// * `title` - The title name to search for.
  /// * `year` - The year to search for titles in.
  fn cookies_by_title_and_year(&self, title: &str, year: u16) -> impl Iterator<Item = &C> {
    if let Some(by_year) = self.by_title.get(title) {
      if let Some(cookies) = by_year.get(&year) {
        return cookies.iter();
      }
    }

    [].iter()
  }

  /// Search for titles by keywords.
  ///
  /// # Arguments
  ///
  /// * `keywords` - Keywords to search for in title names.
  fn cookies_by_keywords<'a, 'k>(&'a self, keywords: &'k [&str]) -> impl Iterator<Item = &'a C> {
    let searcher = AhoCorasickBuilder::new().match_kind(ACMatchKind::LeftmostFirst).build(keywords);
    let keywords_len = keywords.len();
    self
      .by_title
      .iter()
      .filter(move |&(title, _)| {
        let matches: FnvHashSet<_> = searcher.find_iter(title).map(|mat| mat.pattern()).collect();
        matches.len() == keywords_len
      })
      .flat_map(|(_, by_year)| by_year.values())
      .flatten()
  }

  /// Search for titles by keywords and year.
  ///
  /// # Arguments
  ///
  /// * `keywords` - Keywords to search for in title names.
  /// * `year` - The year to search for titles in.
  fn cookies_by_keywords_and_year<'a, 'k>(
    &'a self,
    keywords: &'k [&str],
    year: u16,
  ) -> impl Iterator<Item = &'a C> {
    let searcher = AhoCorasickBuilder::new().match_kind(ACMatchKind::LeftmostFirst).build(keywords);
    let keywords_len = keywords.len();
    self
      .by_title
      .iter()
      .filter(move |&(title, _)| {
        let matches: FnvHashSet<_> = searcher.find_iter(title).map(|mat| mat.pattern()).collect();
        matches.len() == keywords_len
      })
      .filter_map(move |(_, by_year)| by_year.get(&year))
      .flatten()
  }

  /// Insert a cookie with the given title ID.
  ///
  /// # Arguments
  ///
  /// * `id` - Title ID to insert the cookie for.
  /// * `cookie` - Cookie to be inserted.
  fn insert_by_id(&mut self, id: &TitleId, cookie: C) -> bool {
    self.by_id.insert(id.as_usize(), cookie).is_none()
  }

  /// Insert cookie for the title with the given name and year.
  ///
  /// # Arguments
  ///
  /// * `title` - Name of the title to be inserted.
  /// * `year` - Release year of the title to be inserted.
  /// * `cookie` - Cookie to be inserted.
  fn insert_by_title_and_year(&mut self, title: String, year: Option<u16>, cookie: C) {
    if let Some(year) = year {
      self.by_title.entry(title).or_default().entry(year).or_default().push(cookie);
    } else {
      self.by_title.entry(title).or_default().entry(0).or_default().push(cookie);
    }
  }
}

impl<C: Into<usize> + Copy> DbImpl<C> {
  /// Find title by IMDB ID.
  ///
  /// # Arguments
  ///
  /// * `id` - Title ID to find.
  pub(crate) fn by_id(&self, id: &TitleId) -> Option<&Title> {
    self.cookie_by_id(id).map(|&cookie| &self[cookie])
  }

  /// Find titles by name.
  ///
  /// # Arguments
  ///
  /// * `title` - Title name to search for.
  pub(crate) fn by_title<'a>(&'a self, title: &str) -> Box<dyn Iterator<Item = &Title> + 'a> {
    if let Some(by_year) = self.by_title.get(title) {
      return Box::new(by_year.values().flatten().map(|&cookie| &self[cookie]));
    }

    Box::new(std::iter::empty())
  }

  /// Find titles by name and year.
  ///
  /// # Arguments
  ///
  /// * `title` - Title name to search for.
  /// * `year` - The year to search for titles in.
  pub(crate) fn by_title_and_year(&self, title: &str, year: u16) -> impl Iterator<Item = &Title> {
    self.cookies_by_title_and_year(title, year).map(|&cookie| &self[cookie])
  }

  /// Search for titles by keywords.
  ///
  /// # Arguments
  ///
  /// * `keywords` - Keywords to search for.
  pub(crate) fn by_keywords<'a, 'k>(&'a self, keywords: &'k [&str]) -> impl Iterator<Item = &'a Title> {
    self.cookies_by_keywords(keywords).map(|&cookie| &self[cookie])
  }

  /// Search for titles by keywords and year.
  ///
  /// # Arguments
  ///
  /// * `keywords` - Keywords to search for.
  /// * `year` - The year to search for titles in.
  pub(crate) fn by_keywords_and_year<'a, 'k>(
    &'a self,
    keywords: &'k [&str],
    year: u16,
  ) -> impl Iterator<Item = &'a Title> {
    self.cookies_by_keywords_and_year(keywords, year).map(|&cookie| &self[cookie])
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

    let mut movies_storage = Vec::new();
    let mut series_storage = Vec::new();
    Db::to_binary(ratings_reader, basics_reader, &mut movies_storage, &mut series_storage).unwrap();

    let mut basics_reader = make_basics_reader();
    let ratings_reader = make_ratings_reader();

    let ratings = Ratings::from_tsv(ratings_reader).unwrap();

    let mut basics_data = String::new();
    basics_reader.read_to_string(&mut basics_data).unwrap();

    let mut titles_from_tsv = Vec::new();

    let mut tsv_lines_iter = basics_data.lines();

    // Ignore first line.
    tsv_lines_iter.next();

    for line in tsv_lines_iter {
      let title = Title::from_tsv(line.as_bytes(), &ratings).unwrap();
      let title: Option<Title> = title.into();
      let title = title.unwrap();
      titles_from_tsv.push(title);
    }

    let mut titles_from_binary = Vec::new();
    let cursor: &mut &[u8] = &mut movies_storage.as_ref();
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