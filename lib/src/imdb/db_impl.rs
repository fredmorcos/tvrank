#![warn(clippy::all)]

use std::collections::HashMap;
use std::ops::Index;

use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use crate::utils::search::SearchString;

use aho_corasick::AhoCorasick;
use deunicode::deunicode;
use fnv::{FnvHashMap, FnvHashSet};

type ById<C> = FnvHashMap<usize, C>;
type ByYear<C> = FnvHashMap<u16, Vec<C>>;
type ByTitle<C> = HashMap<String, ByYear<C>>;

pub(crate) struct DbImpl<C> {
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
  pub(crate) fn store_title(&mut self, title: Title<'static>) {
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
  pub(crate) fn with_capacity(cap: usize) -> Self {
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
  pub(crate) fn n_titles(&self) -> usize {
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

  /// Search for titles with the given title.
  ///
  /// # Arguments
  ///
  /// * `title` - Title name to search for.
  pub(crate) fn cookies_by_title(&self, title: &SearchString) -> Option<impl Iterator<Item = &C>> {
    self.by_title.get(title.as_str()).map(|by_year| by_year.values().flatten())
  }

  /// Search for titles with the given title and year.
  ///
  /// # Arguments
  ///
  /// * `title` - The title name to search for.
  /// * `year` - The year to search for titles in.
  fn cookies_by_title_and_year(&self, title: &SearchString, year: u16) -> impl Iterator<Item = &C> {
    if let Some(by_year) = self.by_title.get(title.as_str())
      && let Some(cookies) = by_year.get(&year)
    {
      return cookies.iter();
    }

    [].iter()
  }

  /// Search for titles by keywords.
  ///
  /// # Arguments
  ///
  /// * `matcher` - Keyword matcher to use.
  fn cookies_by_keywords(&self, matcher: &AhoCorasick) -> impl Iterator<Item = &C> {
    self
      .by_title
      .iter()
      .filter(move |&(title, _)| {
        let matches: FnvHashSet<_> = matcher.find_iter(title).map(|mat| mat.pattern()).collect();
        matches.len() == matcher.patterns_len()
      })
      .flat_map(|(_, by_year)| by_year.values())
      .flatten()
  }

  /// Search for titles by keywords and year.
  ///
  /// # Arguments
  ///
  /// * `matcher` - Keyword matcher to use.
  /// * `year` - The year to search for titles in.
  fn cookies_by_keywords_and_year(&self, matcher: &AhoCorasick, year: u16) -> impl Iterator<Item = &C> {
    self
      .by_title
      .iter()
      .filter(move |&(title, _)| {
        let matches: FnvHashSet<_> = matcher.find_iter(title).map(|mat| mat.pattern()).collect();
        matches.len() == matcher.patterns_len()
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
  pub(crate) fn by_id(&'_ self, id: &TitleId) -> Option<&'_ Title<'_>> {
    self.cookie_by_id(id).map(|&cookie| &self[cookie])
  }

  /// Find titles by name.
  ///
  /// # Arguments
  ///
  /// * `title` - Title name to search for.
  pub(crate) fn by_title<'a: 'b, 'b>(
    &'a self,
    title: &'b SearchString,
  ) -> Box<dyn Iterator<Item = &'a Title<'a>> + 'b> {
    if let Some(cookies) = self.cookies_by_title(title) {
      return Box::new(cookies.map(|&cookie| &self[cookie]));
    }

    Box::new(std::iter::empty())
  }

  /// Find titles by name and year.
  ///
  /// # Arguments
  ///
  /// * `title` - Title name to search for.
  /// * `year` - The year to search for titles in.
  pub(crate) fn by_title_and_year<'a: 'b, 'b>(
    &'a self,
    title: &'b SearchString,
    year: u16,
  ) -> impl Iterator<Item = &'a Title<'a>> {
    self.cookies_by_title_and_year(title, year).map(|&cookie| &self[cookie])
  }

  /// Search for titles by keywords.
  ///
  /// # Arguments
  ///
  /// * `matcher` - Keyword matcher to use.
  pub(crate) fn by_keywords<'a: 'b, 'b>(
    &'a self,
    matcher: &'b AhoCorasick,
  ) -> impl Iterator<Item = &'a Title<'a>> + 'b {
    self.cookies_by_keywords(matcher).map(|&cookie| &self[cookie])
  }

  /// Search for titles by keywords and year.
  ///
  /// # Arguments
  ///
  /// * `matcher` - Keyword matcher to use.
  /// * `year` - The year to search for titles in.
  pub(crate) fn by_keywords_and_year<'a: 'b, 'b>(
    &'a self,
    matcher: &'b AhoCorasick,
    year: u16,
  ) -> impl Iterator<Item = &'a Title<'a>> + 'b {
    self.cookies_by_keywords_and_year(matcher, year).map(|&cookie| &self[cookie])
  }
}

#[cfg(test)]
mod test_db_impl {
  use std::io::BufRead;

  use aho_corasick::{AhoCorasickBuilder, MatchKind as ACMatchKind};

  use crate::imdb::db_impl::DbImpl;
  use crate::imdb::ratings::Ratings;
  use crate::imdb::testdata::{make_basics_reader, make_ratings_reader};
  use crate::imdb::title::{Title, TsvAction};
  use crate::imdb::title_id::TitleId;
  use crate::utils::search::SearchString;

  fn make_db_impl() -> DbImpl<usize> {
    let mut db_impl = DbImpl::with_capacity(10);
    let ratings = Ratings::from_tsv(make_ratings_reader()).unwrap();
    for line in make_basics_reader().lines().skip(1) {
      let line = Box::leak(line.unwrap().into_boxed_str());
      match Title::from_tsv(line.as_bytes(), &ratings).unwrap() {
        TsvAction::Skip => {}
        TsvAction::Movie(title) => db_impl.store_title(title),
        TsvAction::Series(_) => panic!("Invalid test contents"),
      }
    }
    db_impl
  }

  #[test]
  fn test_by_title() {
    let db_impl = make_db_impl();
    let title = SearchString::try_from("Corbett and Courtney Before the Kinetograph").unwrap();
    let titles: Vec<_> = db_impl.by_title(&title).collect();
    assert_eq!(titles.len(), 1);
    let title = titles[0];
    assert_eq!(title.title_id(), &TitleId::try_from("tt0000007").unwrap());
    assert_eq!(title.primary_title(), "Corbett and Courtney Before the Kinetograph");
  }

  #[test]
  fn test_by_title_and_year() {
    let db_impl = make_db_impl();
    let title = SearchString::try_from("Corbett and Courtney Before the Kinetograph").unwrap();
    let titles: Vec<_> = db_impl.by_title_and_year(&title, 1894).collect();
    assert_eq!(titles.len(), 1);
    let title = titles[0];
    assert_eq!(title.title_id(), &TitleId::try_from("tt0000007").unwrap());
    assert_eq!(title.primary_title(), "Corbett and Courtney Before the Kinetograph");
  }

  #[test]
  fn test_by_keywords() {
    let keywords = &[SearchString::try_from("Corbett").unwrap(), SearchString::try_from("Courtney").unwrap()];
    let matcher = AhoCorasickBuilder::new()
      .match_kind(ACMatchKind::LeftmostFirst)
      .build(keywords)
      .unwrap();
    let db_impl = make_db_impl();
    let titles: Vec<_> = db_impl.by_keywords(&matcher).collect();
    assert_eq!(titles.len(), 1);
    let title = titles[0];
    assert_eq!(title.title_id(), &TitleId::try_from("tt0000007").unwrap());
    assert_eq!(title.primary_title(), "Corbett and Courtney Before the Kinetograph");
  }

  #[test]
  fn test_by_keywords_and_year() {
    let keywords = &[SearchString::try_from("Corbett").unwrap(), SearchString::try_from("Courtney").unwrap()];
    let matcher = AhoCorasickBuilder::new()
      .match_kind(ACMatchKind::LeftmostFirst)
      .build(keywords)
      .unwrap();
    let db_impl = make_db_impl();
    let titles: Vec<_> = db_impl.by_keywords_and_year(&matcher, 1894).collect();
    assert_eq!(titles.len(), 1);
    let title = titles[0];
    assert_eq!(title.title_id(), &TitleId::try_from("tt0000007").unwrap());
    assert_eq!(title.primary_title(), "Corbett and Courtney Before the Kinetograph");
  }
}
