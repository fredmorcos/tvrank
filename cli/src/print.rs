#![warn(clippy::all)]

use crate::search::SearchRes;

use tvrank::imdb::{ImdbQuery, ImdbTitle};

use humantime::format_duration;
use prettytable::{color, format, Attr, Cell, Row, Table};
use reqwest::Url;
use serde::Serialize;
use thiserror::Error;
use truncatable::Truncatable;

#[derive(Debug, Error)]
#[error("Output printing error")]
pub enum Err {
  #[error("JSON output error: {0}")]
  Json(#[from] serde_json::Error),
  #[error("YAML output error: {0}")]
  Yaml(#[from] serde_yaml::Error),
  #[error("Table output error: {0}")]
  Table(#[from] url::ParseError),
}

#[derive(Debug, Clone, clap::ArgEnum)]
pub enum OutputFormat {
  Json,
  Table,
  Yaml,
}

#[derive(Serialize)]
struct OutputWrapper<'search_res, 'a, 'storage> {
  movies: Option<&'search_res [&'a ImdbTitle<'storage>]>,
  series: Option<&'search_res [&'a ImdbTitle<'storage>]>,
}

impl<'search_res, 'a, 'storage> OutputWrapper<'search_res, 'a, 'storage> {
  fn new(
    movies: Option<&'search_res [&'a ImdbTitle<'storage>]>,
    series: Option<&'search_res [&'a ImdbTitle<'storage>]>,
  ) -> Self {
    Self { movies, series }
  }
}

pub trait Printer {
  type Error;

  fn get_format(&self) -> OutputFormat;

  fn print(
    &self,
    movies: Option<SearchRes>,
    series: Option<SearchRes>,
    imdb_url: &Url,
    search_terms: Option<&str>,
  ) -> Result<(), Self::Error>;
}

pub struct JsonPrinter;

impl JsonPrinter {
  #[must_use]
  pub fn new() -> Self {
    Self
  }
}

impl Printer for JsonPrinter {
  type Error = Err;

  fn get_format(&self) -> OutputFormat {
    OutputFormat::Json
  }

  fn print(
    &self,
    mut movies: Option<SearchRes>,
    mut series: Option<SearchRes>,
    _imdb_url: &Url,
    _search_terms: Option<&str>,
  ) -> Result<(), Self::Error> {
    let movie_results = movies.as_mut().map(|movies| movies.top_sorted_results());
    let series_results = series.as_mut().map(|series| series.top_sorted_results());
    println!("{}", serde_json::to_string_pretty(&OutputWrapper::new(movie_results, series_results))?);
    Ok(())
  }
}

pub struct YamlPrinter;

impl YamlPrinter {
  #[must_use]
  pub fn new() -> Self {
    Self
  }
}

impl Printer for YamlPrinter {
  type Error = Err;

  fn get_format(&self) -> OutputFormat {
    OutputFormat::Yaml
  }

  fn print(
    &self,
    mut movies: Option<SearchRes>,
    mut series: Option<SearchRes>,
    _imdb_url: &Url,
    _search_terms: Option<&str>,
  ) -> Result<(), Self::Error> {
    let movie_results = movies.as_mut().map(|movies| movies.top_sorted_results());
    let series_results = series.as_mut().map(|series| series.top_sorted_results());
    println!("{}", serde_yaml::to_string(&OutputWrapper::new(movie_results, series_results,))?);
    Ok(())
  }
}

#[derive(Clone)]
pub struct TablePrinter {
  color: bool,
}

impl Printer for TablePrinter {
  type Error = Err;

  fn get_format(&self) -> OutputFormat {
    OutputFormat::Table
  }

  fn print(
    &self,
    movies: Option<SearchRes>,
    series: Option<SearchRes>,
    imdb_url: &Url,
    search_terms: Option<&str>,
  ) -> Result<(), Self::Error> {
    if let Some(movies) = movies {
      self.print_results(movies, imdb_url, ImdbQuery::Movies, search_terms)?;
    }
    if let Some(series) = series {
      self.print_results(series, imdb_url, ImdbQuery::Series, search_terms)?;
    }

    Ok(())
  }
}

impl TablePrinter {
  #[must_use]
  pub fn new(color: bool) -> Self {
    Self { color }
  }

  fn print_results(
    &self,
    mut results: SearchRes,
    imdb_url: &Url,
    query: ImdbQuery,
    search_terms: Option<&str>,
  ) -> Result<(), Err> {
    if results.is_empty() {
      if let Some(search_terms) = search_terms {
        eprintln!("No {} matches found for `{search_terms}`", query);
      } else {
        eprintln!("No {} matches found", query);
      }
    } else {
      let num = results.total_len();
      let matches = if num == 1 {
        "match"
      } else {
        "matches"
      };

      if let Some(search_terms) = search_terms {
        if results.is_truncated() {
          println!(
            "Found {num} {} {matches} for `{search_terms}`, {} will be displayed:",
            query,
            results.len()
          )
        } else {
          println!("Found {num} {} {matches} for `{search_terms}`:", query)
        }
      } else if results.is_truncated() {
        println!("Found {num} {} {matches}, {} will be displayed:", query, results.len());
      } else {
        println!("Found {num} {} {matches}:", query);
      }

      let mut table = create_table(self.color);

      for res in results.top_sorted_results() {
        let row = self.create_table_row(res, imdb_url)?;
        table.add_row(row);
      }
      table.printstd();
      println!();
    }

    Ok(())
  }

  fn create_table_row(&self, title: &ImdbTitle, imdb_url: &Url) -> Result<Row, Err> {
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

      let mut rating_cell = Cell::new(rating_text);
      if self.color {
        rating_cell = rating_cell.with_style(match rating {
          rating if rating.rating() >= 70 => GREEN,
          rating if (60..70).contains(&rating.rating()) => YELLOW,
          _ => RED,
        });
      }

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

    let url = imdb_url.join(&format!("{}", title_id))?;
    row.add_cell(Cell::new(url.as_str()));

    Ok(row)
  }
}

fn create_table(color: bool) -> Table {
  let mut table = Table::new();

  let table_format = format::FormatBuilder::new()
    .column_separator('│')
    .borders('│')
    .padding(1, 1)
    .build();

  table.set_format(table_format);

  #[macro_export]
  macro_rules! make_bold {
    ($title: expr, $color: expr) => {
      match $color {
        true => Cell::new($title).with_style(Attr::Bold),
        false => Cell::new($title),
      }
    };
  }

  table.add_row(Row::new(vec![
    make_bold!("Primary Title", color),
    make_bold!("Original Title", color),
    make_bold!("Year", color),
    make_bold!("Rating", color),
    make_bold!("Votes", color),
    make_bold!("Runtime", color),
    make_bold!("Genres", color),
    make_bold!("Type", color),
    make_bold!("IMDB ID", color),
    make_bold!("IMDB Link", color),
  ]));

  table
}
