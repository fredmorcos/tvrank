#![warn(clippy::all)]

use atoi::atoi;
use derive_more::Display;
use directories::ProjectDirs;
use humantime::format_duration;
use log::{debug, error, info, trace, warn};
use prettytable::{color, format, Attr, Cell, Row, Table};
use regex::Regex;
use reqwest::Url;
use std::error::Error;
use std::fs;
use std::time::{Duration, Instant};
use structopt::StructOpt;
use titlecase::titlecase;
use tvrank::imdb::{Imdb, ImdbErr};
use tvrank::Res;

#[derive(Debug, Display)]
#[display(fmt = "{}")]
enum TvRankErr {
  #[display(fmt = "Invalid title format, must match TITLE (YYYY)")]
  Input,

  #[display(fmt = "Could not read title from input")]
  Title,

  #[display(fmt = "Could not read year from input")]
  Year,

  #[display(fmt = "Could not find cache directory")]
  CacheDir,
}

impl TvRankErr {
  fn input<T>() -> Res<T> {
    Err(Box::new(TvRankErr::Input))
  }

  fn title<T>() -> Res<T> {
    Err(Box::new(TvRankErr::Title))
  }

  fn year<T>() -> Res<T> {
    Err(Box::new(TvRankErr::Year))
  }

  fn cache_dir<T>() -> Res<T> {
    Err(Box::new(TvRankErr::CacheDir))
  }
}

impl Error for TvRankErr {}

fn parse_name_and_year(input: &str) -> Res<(&str, &str)> {
  debug!("Input: {}", input);

  let regex = Regex::new(r"^(.+)\s+\((\d{4})\)$")?;
  let captures = if let Some(captures) = regex.captures(input) {
    captures
  } else {
    return TvRankErr::input();
  };

  let title = if let Some(title) = captures.get(1) {
    debug!("{:?}", title);
    title.as_str()
  } else {
    return TvRankErr::title();
  };

  let year = if let Some(year) = captures.get(2) {
    debug!("{:?}", year);
    year.as_str()
  } else {
    return TvRankErr::year();
  };

  Ok((title, year))
}

fn create_project() -> Res<ProjectDirs> {
  let prj = ProjectDirs::from("com.fredmorcos", "Fred Morcos", "tvrank");
  if let Some(prj) = prj {
    Ok(prj)
  } else {
    TvRankErr::cache_dir()
  }
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Query information about movies and series")]
#[structopt(author = "Fred Morcos <fm@fredmorcos.com>")]
struct Opt {
  /// Verbose output (can be specified multiple times).
  #[structopt(short, long, parse(from_occurrences))]
  verbose: u8,

  /// Whether to lookup series instead of movies.
  #[structopt(short, long)]
  series: bool,

  /// Input in the format of "TITLE (YYYY)".
  #[structopt(name = "INPUT")]
  input: String,
}

fn run(opt: &Opt) -> Res<()> {
  let project = create_project()?;
  let cache_dir = project.cache_dir();
  info!("Cache directory: {}", cache_dir.display());

  fs::create_dir_all(cache_dir)?;
  debug!("Created cache directory");

  let input = opt.input.trim();
  let (name, year) = match parse_name_and_year(input) {
    Ok((title, year)) => {
      let year = atoi::<u16>(year.as_bytes()).ok_or(ImdbErr::IdNumber)?;
      info!("Title: `{}`, Year: `{}`", title, year);
      (title, Some(year))
    }
    Err(err) => {
      debug!("Failed to parse title and year: {}", err);
      debug!("Using full input as title");
      info!("Title: `{}`", input);
      (input, None)
    }
  };

  let start_time = Instant::now();
  let imdb = Imdb::new(cache_dir)?;
  let duration = Instant::now().duration_since(start_time);
  info!("Loaded IMDB in {}", format_duration(duration));

  let query_fn = if opt.series {
    Imdb::series
  } else {
    Imdb::movie
  };

  let names_fn = if opt.series {
    Imdb::series_names
  } else {
    Imdb::movie_names
  };

  let start_time = Instant::now();
  // TODO: Need to properly shutdown the multi-threaded service before exiting the main
  // thread. Replace the ? with a proper match on the error, print the error and wait for
  // the threads to shutdown. This will also require a shutdown() method on the Imdb
  // Service.
  let mut results = query_fn(&imdb, name.to_ascii_lowercase().as_bytes(), year)?;
  results.sort_unstable();

  if results.is_empty() {
    println!("No results");
  } else {
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

    fn humantitle(title: &[u8]) -> String {
      titlecase(unsafe { std::str::from_utf8_unchecked(title) })
    }

    const IMDB: &str = "https://www.imdb.com/title/";
    let imdb_url = Url::parse(IMDB)?;

    for result in results {
      let mut row = Row::new(vec![]);

      let title_id = result.title_id();
      let titles = names_fn(&imdb, title_id)?;

      match titles[..] {
        [] => {
          row.add_cell(Cell::new("N/A"));
          row.add_cell(Cell::new(""));
        }
        [ptitle] => {
          row.add_cell(Cell::new(&humantitle(ptitle)));
          row.add_cell(Cell::new(""));
        }
        [ptitle, otitle] => {
          row.add_cell(Cell::new(&humantitle(ptitle)));
          row.add_cell(Cell::new(&humantitle(otitle)));
        }
        [ptitle, otitle, ..] => {
          for title in titles {
            debug!("Title with ID {} has name: {}", title_id, &humantitle(title));
          }
          row.add_cell(Cell::new(&humantitle(ptitle)));
          row.add_cell(Cell::new(&humantitle(otitle)));
        }
      }

      if let Some(year) = result.start_year() {
        row.add_cell(Cell::new(&format!("{}", year)));
      } else {
        row.add_cell(Cell::new(""));
      }

      if let Some(&(rating, votes)) = imdb.rating(title_id) {
        let rating_text = &format!("{}/100", rating);

        let rating_cell = if rating >= 70 {
          Cell::new(rating_text).with_style(Attr::ForegroundColor(color::GREEN))
        } else if (60..70).contains(&rating) {
          Cell::new(rating_text).with_style(Attr::ForegroundColor(color::YELLOW))
        } else {
          Cell::new(rating_text).with_style(Attr::ForegroundColor(color::RED))
        };

        row.add_cell(rating_cell);
        row.add_cell(Cell::new(&format!("{}", votes)));
      } else {
        row.add_cell(Cell::new(""));
        row.add_cell(Cell::new(""));
      }

      if let Some(runtime) = result.runtime_minutes() {
        row.add_cell(Cell::new(
          &format_duration(Duration::from_secs(u64::from(runtime) * 60)).to_string(),
        ));
      } else {
        row.add_cell(Cell::new(""));
      }

      row.add_cell(Cell::new(&format!("{}", result.genres())));
      row.add_cell(Cell::new(&format!("{}", result.title_type())));
      row.add_cell(Cell::new(&format!("{}", title_id)));

      let url = imdb_url.join(&format!("tt{}", title_id))?;
      row.add_cell(Cell::new(url.as_str()));

      table.add_row(row);
    }

    table.printstd();
  }

  let duration = Instant::now().duration_since(start_time);
  info!("IMDB query took {}", format_duration(duration));

  std::mem::forget(imdb);

  Ok(())
}

fn main() {
  let start_time = Instant::now();
  let opt = Opt::from_args();

  let log_level = match opt.verbose {
    0 => log::LevelFilter::Off,
    1 => log::LevelFilter::Error,
    2 => log::LevelFilter::Warn,
    3 => log::LevelFilter::Info,
    4 => log::LevelFilter::Debug,
    _ => log::LevelFilter::Trace,
  };

  let logger_available =
    if let Err(e) = env_logger::Builder::new().filter_level(log_level).try_init() {
      eprintln!("Error initializing logger: {}", e);
      false
    } else {
      true
    };

  error!("Error output enabled.");
  warn!("Warning output enabled.");
  info!("Info output enabled.");
  debug!("Debug output enabled.");
  trace!("Trace output enabled.");

  if let Err(e) = run(&opt) {
    if logger_available {
      error!("Error: {}", e);
    } else {
      eprintln!("Error: {}", e);
    }
  }

  let total_time = Instant::now().duration_since(start_time);

  if logger_available {
    info!("Total time: {}", format_duration(total_time));
  } else {
    eprintln!("Total time: {}", format_duration(total_time));
  }
}
