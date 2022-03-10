#![warn(clippy::all)]

use humantime::format_duration;
use prettytable::{color, format, Attr, Cell, Row, Table};
use reqwest::Url;
use std::cmp::Ordering;
use std::ops::{Deref, DerefMut};
use truncatable::Truncatable;
use tvrank::imdb::{ImdbQuery, ImdbTitle};
use tvrank::Res;

pub struct SearchRes<'a, 'storage, 'url> {
  results: Vec<&'a ImdbTitle<'storage>>,
  sort_by_year: bool,
  top: Option<usize>,
  imdb_url: &'url Url,
  query: ImdbQuery,
}

impl<'a, 'storage, 'url> AsRef<[&'a ImdbTitle<'storage>]> for SearchRes<'a, 'storage, 'url> {
  fn as_ref(&self) -> &[&'a ImdbTitle<'storage>] {
    self.results.as_ref()
  }
}

impl<'a, 'storage, 'url> AsMut<[&'a ImdbTitle<'storage>]> for SearchRes<'a, 'storage, 'url> {
  fn as_mut(&mut self) -> &mut [&'a ImdbTitle<'storage>] {
    self.results.as_mut()
  }
}

impl<'a, 'storage, 'url> Deref for SearchRes<'a, 'storage, 'url> {
  type Target = Vec<&'a ImdbTitle<'storage>>;

  fn deref(&self) -> &Self::Target {
    &self.results
  }
}

impl<'a, 'storage, 'url> DerefMut for SearchRes<'a, 'storage, 'url> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.results
  }
}

impl<'a, 'storage, 'url> IntoIterator for SearchRes<'a, 'storage, 'url> {
  type Item = &'a ImdbTitle<'storage>;

  type IntoIter = std::vec::IntoIter<Self::Item>;

  fn into_iter(self) -> Self::IntoIter {
    self.results.into_iter()
  }
}

impl<'a, 'storage, 'url> SearchRes<'a, 'storage, 'url> {
  pub fn new_movies(imdb_url: &'url Url, sort_by_year: bool, top: Option<usize>) -> Self {
    Self { results: Vec::new(), imdb_url, sort_by_year, top, query: ImdbQuery::Movies }
  }

  pub fn new_series(imdb_url: &'url Url, sort_by_year: bool, top: Option<usize>) -> Self {
    Self { results: Vec::new(), imdb_url, sort_by_year, top, query: ImdbQuery::Series }
  }

  pub fn extend(&mut self, iter: impl IntoIterator<Item = &'a ImdbTitle<'storage>>) {
    self.results.extend(iter.into_iter())
  }

  pub fn print(mut self, search_terms: Option<&str>) -> Res<()> {
    self.sort_results();
    self.print_table(search_terms)?;
    Ok(())
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

  fn print_table(self, search_terms: Option<&str>) -> Res<()> {
    if self.results.is_empty() {
      if let Some(search_terms) = search_terms {
        eprintln!("No {} matches found for `{search_terms}`", self.query);
      } else {
        eprintln!("No {} matches found", self.query);
      }
    } else {
      let num = self.results.len();
      let matches = if num == 1 {
        "match"
      } else {
        "matches"
      };

      if let Some(search_terms) = search_terms {
        match self.top {
          Some(n) => {
            println!(
              "Found {num} {} {matches} for `{search_terms}`, {} will be displayed:",
              self.query,
              num.min(n)
            )
          }
          None => println!("Found {num} {} {matches} for `{search_terms}`:", self.query),
        };
      } else {
        match self.top {
          Some(n) => println!("Found {num} {} {matches}, {} will be displayed:", self.query, num.min(n)),
          None => println!("Found {num} {} {matches}:", self.query),
        };
      }

      let mut table = create_table();

      let results = match self.top {
        Some(n) => &self.results[0..num.min(n)],
        None => &self.results,
      };

      for &res in results {
        let row = self.create_table_row(res)?;
        table.add_row(row);
      }
      table.printstd();
      println!();
    }

    Ok(())
  }

  fn create_table_row(&self, title: &ImdbTitle) -> Res<Row> {
    static GREEN: Attr = Attr::ForegroundColor(color::GREEN);
    static YELLOW: Attr = Attr::ForegroundColor(color::YELLOW);
    static RED: Attr = Attr::ForegroundColor(color::RED);

    let mut row = Row::new(vec![]);

    row.add_cell(Cell::new(&Truncatable::from(title.primary_title()).truncate(50)));

    if let Some(original_title) = title.original_title() {
      row.add_cell(Cell::new(&Truncatable::from(original_title).truncate(30)));
    } else {
      row.add_cell(Cell::new(""));
    }

    if let Some(year) = title.start_year() {
      row.add_cell(Cell::new(&format!("{}", year)));
    } else {
      row.add_cell(Cell::new(""));
    }

    if let Some(rating) = title.rating() {
      let rating_text = &format!("{}/100", rating.rating());

      let rating_cell = Cell::new(rating_text).with_style(match rating {
        rating if rating.rating() >= 70 => GREEN,
        rating if (60..70).contains(&rating.rating()) => YELLOW,
        _ => RED,
      });

      row.add_cell(rating_cell);
      row.add_cell(Cell::new(&format!("{}", rating.votes())));
    } else {
      row.add_cell(Cell::new(""));
      row.add_cell(Cell::new(""));
    }

    if let Some(runtime) = title.runtime() {
      row.add_cell(Cell::new(&format_duration(runtime).to_string()));
    } else {
      row.add_cell(Cell::new(""));
    }

    row.add_cell(Cell::new(&format!("{}", title.genres())));
    row.add_cell(Cell::new(&format!("{}", title.title_type())));

    let title_id = title.title_id();
    row.add_cell(Cell::new(&format!("{}", title_id)));

    let url = self.imdb_url.join(&format!("{}", title_id))?;
    row.add_cell(Cell::new(url.as_str()));

    Ok(row)
  }
}

fn create_table() -> Table {
  let mut table = Table::new();

  let table_format = format::FormatBuilder::new()
    .column_separator('│')
    .borders('│')
    .padding(1, 1)
    .build();

  table.set_format(table_format);

  table.add_row(Row::new(vec![
    Cell::new("Primary Title").with_style(Attr::Bold),
    Cell::new("Original Title").with_style(Attr::Bold),
    Cell::new("Year").with_style(Attr::Bold),
    Cell::new("Rating").with_style(Attr::Bold),
    Cell::new("Votes").with_style(Attr::Bold),
    Cell::new("Runtime").with_style(Attr::Bold),
    Cell::new("Genres").with_style(Attr::Bold),
    Cell::new("Type").with_style(Attr::Bold),
    Cell::new("IMDB ID").with_style(Attr::Bold),
    Cell::new("IMDB Link").with_style(Attr::Bold),
  ]));

  table
}
