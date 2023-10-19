#![warn(clippy::all)]

use std::cmp::Ordering;
use std::ops::{Deref, DerefMut};
use tvrank::imdb::ImdbTitle;

pub struct SearchRes<'a, 'storage> {
  results: Vec<&'a ImdbTitle<'storage>>,
  sort_by_year: bool,
  top: Option<usize>,
}

impl<'a, 'storage> AsRef<[&'a ImdbTitle<'storage>]> for SearchRes<'a, 'storage> {
  fn as_ref(&self) -> &[&'a ImdbTitle<'storage>] {
    self.results.as_ref()
  }
}

impl<'a, 'storage> AsMut<[&'a ImdbTitle<'storage>]> for SearchRes<'a, 'storage> {
  fn as_mut(&mut self) -> &mut [&'a ImdbTitle<'storage>] {
    self.results.as_mut()
  }
}

impl<'a, 'storage> Deref for SearchRes<'a, 'storage> {
  type Target = Vec<&'a ImdbTitle<'storage>>;

  fn deref(&self) -> &Self::Target {
    &self.results
  }
}

impl<'a, 'storage> DerefMut for SearchRes<'a, 'storage> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.results
  }
}

impl<'a, 'storage> IntoIterator for SearchRes<'a, 'storage> {
  type Item = &'a ImdbTitle<'storage>;

  type IntoIter = std::vec::IntoIter<Self::Item>;

  fn into_iter(self) -> Self::IntoIter {
    self.results.into_iter()
  }
}

impl<'a, 'storage> SearchRes<'a, 'storage> {
  pub fn new(sort_by_year: bool, top: Option<usize>) -> Self {
    Self { results: Vec::new(), sort_by_year, top }
  }

  pub fn total_len(&self) -> usize {
    self.results.len()
  }

  pub fn len(&self) -> usize {
    match self.top {
      Some(top) => top.min(self.total_len()),
      None => self.total_len(),
    }
  }

  pub fn is_truncated(&self) -> bool {
    match self.top {
      Some(top) => top <= self.total_len(),
      None => false,
    }
  }

  pub fn extend(&mut self, iter: impl IntoIterator<Item = &'a ImdbTitle<'storage>>) {
    self.results.extend(iter)
  }

  pub fn top_sorted_results(&mut self) -> &[&'a ImdbTitle<'storage>] {
    self.sort_results();
    &self.results[0..self.len()]
  }

  fn sort_results(&mut self) {
    if self.sort_by_year {
      self.results.sort_unstable_by(|a, b| {
        match b.start_year().cmp(&a.start_year()) {
          Ordering::Equal => {}
          ord => return ord,
        }

        match b.rating().cmp(&a.rating()) {
          Ordering::Equal => {}
          ord => return ord,
        }

        b.primary_title().cmp(a.primary_title())
      })
    } else {
      self.results.sort_unstable_by(|a, b| {
        match b.rating().cmp(&a.rating()) {
          Ordering::Equal => {}
          ord => return ord,
        }

        match b.start_year().cmp(&a.start_year()) {
          Ordering::Equal => {}
          ord => return ord,
        }

        b.primary_title().cmp(a.primary_title())
      })
    }
  }
}
