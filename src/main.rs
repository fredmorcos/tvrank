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
use tvrank::imdb::title::TitleId;
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

fn parse_name_and_year(input: &str) -> Option<(&str, u16)> {
  debug!("Input: {}", input);

  let regex = match Regex::new(r"^(.+)\s+\((\d{4})\)$") {
    Ok(regex) => regex,
    Err(e) => {
      warn!("Could not parse input `{}` as TITLE (YYYY): {}", input, e);
      return None;
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
          Some((title, year_val))
        } else {
          warn!("Could not parse year `{}`", year_match.as_str());
          None
        }
      } else {
        warn!("Could not parse year from {}", input);
        None
      }
    } else {
      warn!("Could not parse title from {}", input);
      None
    }
  } else {
    warn!("Could not parse title and year from {}", input);
    None
  }
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

fn create_output_table() -> Table {
  let mut table = Table::new();

  let table_format =
    format::FormatBuilder::new().column_separator('│').borders('│').padding(1, 1).build();

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

fn create_output_table_row_for_title(title: &Title, imdb_url: &Url) -> Res<Row> {
  static GREEN: Attr = Attr::ForegroundColor(color::GREEN);
  static YELLOW: Attr = Attr::ForegroundColor(color::YELLOW);
  static RED: Attr = Attr::ForegroundColor(color::RED);

  let mut row = Row::new(vec![]);

  row.add_cell(Cell::new(&humantitle(title.imdb_primary_title)));
  row.add_cell(Cell::new(&humantitle(title.imdb_original_title)));

  if let Some(year) = title.imdb_title.start_year() {
    row.add_cell(Cell::new(&format!("{}", year)));
  } else {
    row.add_cell(Cell::new(""));
  }

  if let Some(&(rating, votes)) = title.imdb_rating {
    let rating_text = &format!("{}/100", rating);

    let rating_cell = Cell::new(rating_text).with_style(match rating {
      rating if rating >= 70 => GREEN,
      rating if (60..70).contains(&rating) => YELLOW,
      _ => RED,
    });

    row.add_cell(rating_cell);
    row.add_cell(Cell::new(&format!("{}", votes)));
  } else {
    row.add_cell(Cell::new(""));
    row.add_cell(Cell::new(""));
  }

  if let Some(runtime) = title.imdb_title.runtime_minutes() {
    row.add_cell(Cell::new(
      &format_duration(Duration::from_secs(u64::from(runtime) * 60)).to_string(),
    ));
  } else {
    row.add_cell(Cell::new(""));
  }

  row.add_cell(Cell::new(&format!("{}", title.imdb_title.genres())));
  row.add_cell(Cell::new(&format!("{}", title.imdb_title.title_type())));

  let title_id = title.imdb_title.title_id();
  row.add_cell(Cell::new(&format!("{}", title_id)));

  let url = imdb_url.join(&format!("tt{}", title_id))?;
  row.add_cell(Cell::new(url.as_str()));

  Ok(row)
}

type QueryFn<'a> =
  fn(db: &'a Imdb, name: &[u8], year: Option<u16>) -> Res<Vec<&'a ImdbTitle>>;
type NamesFn<'a> = fn(db: &'a Imdb, id: TitleId) -> Res<Vec<&'a [u8]>>;

fn setup_imdb_db(cache_dir: &Path, series: bool) -> Res<(Imdb, QueryFn, NamesFn)> {
  let start_time = Instant::now();
  let imdb = Imdb::new(cache_dir)?;
  let duration = Instant::now().duration_since(start_time);
  info!("Loaded IMDB database in {}", format_duration(duration));

  let query_fn = if series {
    Imdb::series
  } else {
    Imdb::movie
  };

  let names_fn = if series {
    Imdb::series_names
  } else {
    Imdb::movie_names
  };

  Ok((imdb, query_fn, names_fn))
}

fn imdb_lookup<'a>(
  name: &str,
  year: Option<u16>,
  imdb: &'a Imdb,
  query_fn: QueryFn<'a>,
  names_fn: NamesFn<'a>,
  results: &mut Vec<Title<'a>>,
) -> Res<Vec<Title<'a>>> {
  let qresults = query_fn(imdb, name.to_ascii_lowercase().as_bytes(), year)?;
  let mut indiv_results = vec![];

  let results: &mut Vec<Title> = if !qresults.is_empty() {
    &mut indiv_results
  } else {
    results
  };

  for qresult in qresults {
    let titles = names_fn(imdb, qresult.title_id())?;

    let (imdb_primary_title, imdb_original_title): (&[u8], &[u8]) = match titles[..] {
      [] => {
        debug!("Title with ID {} has no names", qresult.title_id());
        (b"N/A", b"")
      }
      [ptitle] => (ptitle, b""),
      [ptitle, otitle] => (ptitle, otitle),
      [ptitle, otitle, ..] => {
        for title in titles {
          debug!("Title with ID {} has name: {}", qresult.title_id(), &humantitle(title));
        }
        debug!("Only the first two will be used (as primary and original titles)");
        (ptitle, otitle)
      }
    };

    results.push(Title {
      imdb_title: qresult,
      imdb_rating: imdb.rating(qresult.title_id()),
      imdb_primary_title,
      imdb_original_title,
    });
  }

  Ok(indiv_results)
}

fn run(opt: &Opt) -> Res<()> {
  let project = create_project()?;
  let cache_dir = project.cache_dir();
  info!("Cache directory: {}", cache_dir.display());

  fs::create_dir_all(cache_dir)?;
  debug!("Created cache directory");

  const IMDB: &str = "https://www.imdb.com/title/";
  let imdb_url = Url::parse(IMDB)?;

  let queries = match (&opt.title, &opt.dir) {
    (None, None) | (Some(_), Some(_)) => return TvRankErr::title_or_dir(),
    (None, Some(_)) => todo!("Directory lookup is still not implemented"),
    (Some(title), None) => {
      if let Some((name, year)) = parse_name_and_year(title) {
        vec![(name, Some(year))]
      } else {
        vec![(title.as_str(), None)]
      }
    }
  };

  let db = setup_imdb_db(cache_dir, opt.series)?;
  let imdb = db.0;
  let query_fn = db.1;
  let names_fn = db.2;

  let start_time = Instant::now();
  let mut results = Vec::with_capacity(queries.len());

  for &(name, year) in &queries {
    let mut indiv_results =
      imdb_lookup(name, year, &imdb, query_fn, names_fn, &mut results)?;

    if !indiv_results.is_empty() {
      println!(
        "Found {} matches for `{}{}`:",
        indiv_results.len(),
        name,
        if let Some(year) = year {
          format!(" ({})", year)
        } else {
          "".to_string()
        }
      );

      sort_results(&mut indiv_results, opt.sort_by_rating);

      let mut table = create_output_table();

      for indiv_result in indiv_results {
        let row = create_output_table_row_for_title(&indiv_result, &imdb_url)?;
        table.add_row(row);
      }

      table.printstd();
    }
  }

  let duration = Instant::now().duration_since(start_time);
  info!("IMDB query took {}", format_duration(duration));

  if queries.len() > 1 {
    if results.is_empty() {
      println!("No results");
    } else {
      sort_results(&mut results, opt.sort_by_rating);

      let mut table = create_output_table();

      for result in &results {
        let row = create_output_table_row_for_title(result, &imdb_url)?;
        table.add_row(row);
      }

      table.printstd();
    }
  }

  std::mem::forget(results);
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
