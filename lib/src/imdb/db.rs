#![warn(clippy::all)]

use crate::imdb::db_impl::DbImpl;
use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use crate::utils::search::SearchString;

use aho_corasick::AhoCorasick;
use derive_more::{Display, From, Into};

/// Specifies the type of title a query is for. E.g. Movies or Series.
#[derive(Clone, Copy, Display)]
#[display("{}")]
pub enum Query {
  /// Query the database of Movies.
  #[display("movie")]
  Movies,

  /// Query the database of Series.
  #[display("series")]
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
    title: &SearchString,
    query: Query,
  ) -> Box<dyn Iterator<Item = &'a Title<'a>> + 'a> {
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
    title: &SearchString,
    year: u16,
    query: Query,
  ) -> Box<dyn Iterator<Item = &'a Title<'a>> + 'a> {
    match query {
      Query::Movies => Box::new(self.movies.by_title_and_year(title, year)),
      Query::Series => Box::new(self.series.by_title_and_year(title, year)),
    }
  }

  /// Search for titles by keywords.
  ///
  /// # Arguments
  ///
  /// * `matcher` - Keyword matcher to use.
  /// * `query` - Whether to query movies or series.
  pub(crate) fn by_keywords<'a: 'b, 'b>(
    &'a self,
    matcher: &'b AhoCorasick,
    query: Query,
  ) -> Box<dyn Iterator<Item = &'a Title<'a>> + 'b> {
    match query {
      Query::Movies => Box::new(self.movies.by_keywords(matcher)),
      Query::Series => Box::new(self.series.by_keywords(matcher)),
    }
  }

  /// Search for titles by keywords and year.
  ///
  /// # Arguments
  ///
  /// * `matcher` - Keyword matcher to use.
  /// * `year` - The year to search for titles in.
  /// * `query` - Whether to query movies or series.
  pub(crate) fn by_keywords_and_year<'a: 'b, 'b>(
    &'a self,
    matcher: &'b AhoCorasick,
    year: u16,
    query: Query,
  ) -> Box<dyn Iterator<Item = &'a Title<'a>> + 'b> {
    match query {
      Query::Movies => Box::new(self.movies.by_keywords_and_year(matcher, year)),
      Query::Series => Box::new(self.series.by_keywords_and_year(matcher, year)),
    }
  }
}

#[cfg(test)]
mod test_db {
  use std::io::Read;

  use crate::imdb::ratings::Ratings;
  use crate::imdb::testdata::{make_basics_reader, make_ratings_reader};
  use crate::imdb::title::Title;
  use crate::imdb::tsv_import::tsv_import;

  #[test]
  fn test_to_binary() {
    let basics_reader = make_basics_reader();
    let ratings_reader = make_ratings_reader();

    let mut movies_storage = Vec::new();
    let mut series_storage = Vec::new();
    tsv_import(ratings_reader, basics_reader, &mut movies_storage, &mut series_storage).unwrap();

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
