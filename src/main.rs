#![warn(clippy::all)]

use atoi::atoi;
use derive_more::Display;
use directories::ProjectDirs;
use humantime::format_duration;
use log::{debug, error, info, trace, warn};
use prettytable::{color, format, Attr, Cell, Row, Table};
use regex::Regex;
use reqwest::Url;
use std::cmp::Ordering;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use structopt::StructOpt;
use titlecase::titlecase;
use tvrank::imdb::{Imdb, ImdbTitle};
use tvrank::Res;

#[derive(Debug, Display)]
#[display(fmt = "{}")]
enum TvRankErr {
  #[display(fmt = "Must provide either --title or --dir")]
  TitleOrDir,

  #[display(fmt = "Could not find cache directory")]
  CacheDir,
}

impl TvRankErr {
  fn title_or_dir<T>() -> Res<T> {
    Err(Box::new(TvRankErr::TitleOrDir))
  }

  fn cache_dir<T>() -> Res<T> {
    Err(Box::new(TvRankErr::CacheDir))
  }
}

impl Error for TvRankErr {}

fn parse_name_and_year(input: &str) -> (&str, Option<u16>) {
  debug!("Input: {}", input);

  let regex = match Regex::new(r"^(.+)\s+\((\d{4})\)$") {
    Ok(regex) => regex,
    Err(e) => {
      warn!("Failed to parse title: {}", e);
      return (input, None);
    }
  };

  if let Some(captures) = regex.captures(input) {
    if let Some(title_match) = captures.get(1) {
      debug!("Title Match: {:?}", title_match);

      if let Some(year_match) = captures.get(2) {
        debug!("Year Match: {:?}", year_match);

        if let Some(year_val) = atoi::<u16>(year_match.as_str().as_bytes()) {
          let title = title_match.as_str();
          info!("Title: `{}`, Year: `{}`", title, year_val);
          return (title, Some(year_val));
        }
      }
    }
  }

  debug!("Failed to parse title in TITLE (YYYY) format");
  debug!("Using full input as title");
  info!("Title: `{}`", input);
  (input, None)
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
  /// Verbose output (can be specified multiple times)
  #[structopt(short, long, parse(from_occurrences))]
  verbose: u8,

  /// Whether to lookup series instead of movies
  #[structopt(short, long)]
  series: bool,

  /// Sort results by rating, year and title instead of year, rating and title
  #[structopt(short, long)]
  sort_by_rating: bool,

  /// Lookup a single title using "TITLE" or "TITLE (YYYY)"
  #[structopt(short, long, name = "TITLE")]
  title: Option<String>,

  /// Lookup titles from a directory
  #[structopt(short, long, name = "DIR")]
  dir: Option<PathBuf>,
}

struct Title<'db> {
  imdb_title: &'db ImdbTitle,
  imdb_rating: Option<&'db (u8, u64)>,
  imdb_primary_title: &'db [u8],
  imdb_original_title: &'db [u8],
}

fn sort_results(results: &mut Vec<Title>, sort_by_rating: bool) {
  if sort_by_rating {
    results.sort_unstable_by(|a, b| {
      match b.imdb_rating.cmp(&a.imdb_rating) {
        Ordering::Equal => {}
        ord => return ord,
      }
      match b.imdb_title.start_year().cmp(&a.imdb_title.start_year()) {
        Ordering::Equal => {}
        ord => return ord,
      }
      b.imdb_primary_title.cmp(a.imdb_primary_title)
    })
  } else {
    results.sort_unstable_by(|a, b| {
      match b.imdb_title.start_year().cmp(&a.imdb_title.start_year()) {
        Ordering::Equal => {}
        ord => return ord,
      }
      match b.imdb_rating.cmp(&a.imdb_rating) {
        Ordering::Equal => {}
        ord => return ord,
      }
      b.imdb_primary_title.cmp(a.imdb_primary_title)
    })
  }
}

fn humantitle(title: &[u8]) -> String {
  titlecase(unsafe { std::str::from_utf8_unchecked(title) })
}

fn imdb_lookup(
  input: &str,
  cache_dir: &Path,
  is_series: bool,
  sort_by_rating: bool,
) -> Res<()> {
  let (name, year) = parse_name_and_year(input);

  let start_time = Instant::now();
  let imdb = Imdb::new(cache_dir)?;
  let duration = Instant::now().duration_since(start_time);
  info!("Loaded IMDB in {}", format_duration(duration));

  let query_fn = if is_series {
    Imdb::series
  } else {
    Imdb::movie
  };

  let names_fn = if is_series {
    Imdb::series_names
  } else {
    Imdb::movie_names
  };

  let start_time = Instant::now();
  let mut results = Vec::new();

  for result in query_fn(&imdb, name.to_ascii_lowercase().as_bytes(), year)?.into_iter() {
    let titles = names_fn(&imdb, result.title_id())?;

    let (imdb_primary_title, imdb_original_title): (&[u8], &[u8]) = match titles[..] {
      [] => {
        debug!("Title with ID {} has no names", result.title_id());
        (b"N/A", b"")
      }
      [ptitle] => (ptitle, b""),
      [ptitle, otitle] => (ptitle, otitle),
      [ptitle, otitle, ..] => {
        for title in titles {
          debug!("Title with ID {} has name: {}", result.title_id(), &humantitle(title));
        }
        debug!("Only the first two will be used (as primary and original titles)");
        (ptitle, otitle)
      }
    };

    results.push(Title {
      imdb_title: result,
      imdb_rating: imdb.rating(result.title_id()),
      imdb_primary_title,
      imdb_original_title,
    });
  }

  sort_results(&mut results, sort_by_rating);

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

    const IMDB: &str = "https://www.imdb.com/title/";
    let imdb_url = Url::parse(IMDB)?;

    for result in results {
      let mut row = Row::new(vec![]);

      let title_id = result.imdb_title.title_id();

      row.add_cell(Cell::new(&humantitle(result.imdb_primary_title)));
      row.add_cell(Cell::new(&humantitle(result.imdb_original_title)));

      if let Some(year) = result.imdb_title.start_year() {
        row.add_cell(Cell::new(&format!("{}", year)));
      } else {
        row.add_cell(Cell::new(""));
      }

      if let Some(&(rating, votes)) = result.imdb_rating {
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

      if let Some(runtime) = result.imdb_title.runtime_minutes() {
        row.add_cell(Cell::new(
          &format_duration(Duration::from_secs(u64::from(runtime) * 60)).to_string(),
        ));
      } else {
        row.add_cell(Cell::new(""));
      }

      row.add_cell(Cell::new(&format!("{}", result.imdb_title.genres())));
      row.add_cell(Cell::new(&format!("{}", result.imdb_title.title_type())));
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

fn run(opt: &Opt) -> Res<()> {
  let project = create_project()?;
  let cache_dir = project.cache_dir();
  info!("Cache directory: {}", cache_dir.display());

  fs::create_dir_all(cache_dir)?;
  debug!("Created cache directory");

  match (&opt.title, &opt.dir) {
    (None, None) | (Some(_), Some(_)) => return TvRankErr::title_or_dir(),
    (None, Some(_)) => todo!("Directory lookup is still not implemented"),
    (Some(title), None) => imdb_lookup(title, cache_dir, opt.series, opt.sort_by_rating)?,
  }

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
