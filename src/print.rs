use crate::TvRankErr;
use humantime::format_duration;
use prettytable::{color, format, Attr, Cell, Row, Table};
use reqwest::Url;
use serde::de::StdError;
use serde::Serialize;
use std::str::FromStr;
use truncatable::Truncatable;
use tvrank::imdb::{ImdbQuery, ImdbTitle};
use tvrank::Res;

#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
  Json,
  Table,
}

type ParseError = Box<dyn StdError>;

impl FromStr for OutputFormat {
  type Err = ParseError;

  fn from_str(format: &str) -> Result<Self, Self::Err> {
    match format {
      "json" => Ok(OutputFormat::Json),
      "table" => Ok(OutputFormat::Table),
      _ => TvRankErr::not_supported_output(),
    }
  }
}

#[derive(Serialize)]
pub struct OutputWrapper<'a> {
  movies: Option<&'a [&'a ImdbTitle<'a>]>,
  series: Option<&'a [&'a ImdbTitle<'a>]>,
}

pub trait Printer {
  fn get_format(&self) -> OutputFormat;

  fn print(
    &self,
    movies: Option<&[&ImdbTitle]>,
    series: Option<&[&ImdbTitle]>,
    imdb_url: &Url,
    search_terms: Option<&str>,
  ) -> Res<()>;
}

pub struct JsonPrinter {}

impl Printer for JsonPrinter {
  fn get_format(&self) -> OutputFormat {
    OutputFormat::Json
  }

  fn print(
    &self,
    movies: Option<&[&ImdbTitle]>,
    series: Option<&[&ImdbTitle]>,
    _imdb_url: &Url,
    _search_terms: Option<&str>,
  ) -> Res<()> {
    println!("{}", serde_json::to_string_pretty(&OutputWrapper { movies, series })?);

    Ok(())
  }
}

#[derive(Clone)]
pub struct TablePrinter {
  pub(crate) color: bool,
  pub(crate) top: Option<usize>,
}

impl Printer for TablePrinter {
  fn get_format(&self) -> OutputFormat {
    OutputFormat::Table
  }

  fn print(
    &self,
    movies_results: Option<&[&ImdbTitle]>,
    series_results: Option<&[&ImdbTitle]>,
    imdb_url: &Url,
    search_terms: Option<&str>,
  ) -> Res<()> {
    if let Some(movies_results) = movies_results {
      self.print_results(movies_results, imdb_url, ImdbQuery::Movies, search_terms)?;
    }
    if let Some(series_results) = series_results {
      self.print_results(series_results, imdb_url, ImdbQuery::Series, search_terms)?;
    }

    Ok(())
  }
}

impl TablePrinter {
  fn print_results(
    &self,
    results: &[&ImdbTitle],
    imdb_url: &Url,
    query: ImdbQuery,
    search_terms: Option<&str>,
  ) -> Res<()> {
    if results.is_empty() {
      if let Some(search_terms) = search_terms {
        eprintln!("No {} matches found for `{search_terms}`", query);
      } else {
        eprintln!("No {} matches found", query);
      }
    } else {
      let num = results.len();
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
              query,
              num.min(n)
            )
          }
          None => println!("Found {num} {} {matches} for `{search_terms}`:", query),
        };
      } else {
        match self.top {
          Some(n) => println!("Found {num} {} {matches}, {} will be displayed:", query, num.min(n)),
          None => println!("Found {num} {} {matches}:", query),
        };
      }

      let mut table = create_table(self.color);

      for &res in results {
        let row = self.create_table_row(res, imdb_url)?;
        table.add_row(row);
      }
      table.printstd();
      println!();
    }

    Ok(())
  }

  fn create_table_row(&self, title: &ImdbTitle, imdb_url: &Url) -> Res<Row> {
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
