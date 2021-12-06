#![warn(clippy::all)]

mod ui;

use atoi::atoi;
use derive_more::Display;
use directories::ProjectDirs;
use humantime::format_duration;
use indicatif::{HumanBytes, ProgressBar};
use log::{debug, error, info, trace, warn};
use prettytable::{color, format, Attr, Cell, Row, Table};
use regex::Regex;
use reqwest::Url;
use std::cmp::Ordering;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use structopt::StructOpt;
use tvrank::imdb::{Imdb, ImdbStorage, ImdbTitle};
use tvrank::Res;
use ui::{create_progress_bar, create_progress_spinner};
use walkdir::WalkDir;

#[derive(Debug, Display)]
#[display(fmt = "{}")]
enum TvRankErr {
  #[display(fmt = "Could not find cache directory")]
  CacheDir,
}

impl TvRankErr {
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
          debug!("Title: `{}`, Year: `{}`", title, year_val);
          Some((title, year_val))
        } else {
          warn!("Could not parse year `{}`", year_match.as_str());
          None
        }
      } else {
        warn!("Could not parse year from `{}`", input);
        None
      }
    } else {
      warn!("Could not parse title from `{}`", input);
      None
    }
  } else {
    warn!("Could not parse title and year from `{}`", input);
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
  #[structopt(short = "r", long)]
  sort_by_rating: bool,

  #[structopt(subcommand)]
  command: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
  /// Lookup a single title using "TITLE" or "TITLE (YYYY)"
  Title {
    #[structopt(name = "TITLE")]
    title: String,
  },
  /// Lookup titles from a directory
  Dir {
    #[structopt(name = "DIR")]
    dir: PathBuf,
  },
}

fn sort_results(results: &mut Vec<ImdbTitle>, sort_by_rating: bool) {
  if sort_by_rating {
    results.sort_unstable_by(|a, b| {
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
  } else {
    results.sort_unstable_by(|a, b| {
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
  }
}

fn create_output_table() -> Table {
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

fn create_output_table_row_for_title(title: &ImdbTitle, imdb_url: &Url) -> Res<Row> {
  static GREEN: Attr = Attr::ForegroundColor(color::GREEN);
  static YELLOW: Attr = Attr::ForegroundColor(color::YELLOW);
  static RED: Attr = Attr::ForegroundColor(color::RED);

  let mut row = Row::new(vec![]);

  row.add_cell(Cell::new(title.primary_title()));
  row.add_cell(Cell::new(title.original_title()));

  if let Some(year) = title.start_year() {
    row.add_cell(Cell::new(&format!("{}", year)));
  } else {
    row.add_cell(Cell::new(""));
  }

  if let Some(&(rating, votes)) = title.rating() {
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

fn setup_imdb_storage(cache_dir: &Path) -> Res<ImdbStorage> {
  info!("Loading IMDB Databases...");

  // Downloading callbacks.
  let download_init = |db_name: &'_ str, content_len| {
    let msg = format!("Downloading IMDB {} DB", db_name);
    let bar = if let Some(file_length) = content_len {
      info!("IMDB {} DB compressed file size is {}", db_name, HumanBytes(file_length));
      create_progress_bar(msg, file_length)
    } else {
      info!("IMDB {} DB compressed file size is unknown", db_name);
      create_progress_spinner(msg)
    };

    bar
  };
  let download_during = |bar: &ProgressBar, delta| {
    bar.inc(delta);
  };
  let download_finish = |bar: &ProgressBar| {
    bar.finish_and_clear();
  };

  // Extraction callbacks.
  let decomp_init = |db_name: &'_ str| {
    let msg = format!("Decompressing IMDB {} DB...", db_name);
    create_progress_spinner(msg)
  };
  let decomp_during = |spinner: &ProgressBar, delta| {
    spinner.inc(delta);
  };
  let decomp_finish = |spinner: &ProgressBar| {
    spinner.finish_and_clear();
  };

  let imdb_storage = ImdbStorage::new(
    cache_dir,
    download_init,
    download_during,
    download_finish,
    decomp_init,
    decomp_during,
    decomp_finish,
  )?;

  Ok(imdb_storage)
}

fn imdb_lookup<'a>(
  name: &str,
  year: Option<u16>,
  imdb: &'a Imdb,
  query_fn: QueryFn<'a>,
  results: &mut Vec<ImdbTitle<'a, 'a>>,
) -> Res<()> {
  results.extend(query_fn(imdb, &name.to_lowercase(), year)?.into_iter().flatten());
  Ok(())
}

fn display_title(name: &str, year: Option<u16>) -> String {
  format!(
    "{}{}",
    name,
    if let Some(year) = year {
      format!(" ({})", year)
    } else {
      "".to_string()
    }
  )
}

fn single_title<'a>(
  title: &str,
  imdb: &'a Imdb,
  query_fn: QueryFn<'a>,
  imdb_url: &Url,
  sort_by_rating: bool,
) -> Res<()> {
  let (name, year) = if let Some((name, year)) = parse_name_and_year(title) {
    (name, Some(year))
  } else {
    warn!("Going to use `{}` as search query", title);
    (title, None)
  };

  let mut results = vec![];
  imdb_lookup(name, year, imdb, query_fn, &mut results)?;

  if results.is_empty() {
    println!("No matches found for `{}`", display_title(name, year));
  } else {
    println!(
      "Found {} {} for `{}`:",
      results.len(),
      if results.len() == 1 {
        "match"
      } else {
        "matches"
      },
      display_title(name, year)
    );

    sort_results(&mut results, sort_by_rating);

    let mut table = create_output_table();

    for res in &results {
      let row = create_output_table_row_for_title(res, imdb_url)?;
      table.add_row(row);
    }

    table.printstd();
  }

  std::mem::forget(results);
  Ok(())
}

fn titles_dir<'a>(
  dir: &Path,
  imdb: &'a Imdb,
  query_fn: QueryFn<'a>,
  imdb_url: &Url,
  series: bool,
  sort_by_rating: bool,
) -> Res<()> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = vec![];

  let walkdir = WalkDir::new(dir).min_depth(1);
  let walkdir = if series {
    walkdir.max_depth(1)
  } else {
    walkdir
  };

  for entry in walkdir {
    let entry = entry?;

    if entry.file_type().is_dir() {
      if let Some(filename) = entry.path().file_name() {
        let filename = filename.to_string_lossy();

        let (name, year) = if let Some((name, year)) = parse_name_and_year(&filename) {
          at_least_one = true;
          (name, Some(year))
        } else if series {
          (filename.as_ref(), None)
        } else {
          warn!(
            "Skipping `{}` because `{}` does not follow the TITLE (YYYY) format",
            entry.path().display(),
            filename,
          );

          continue;
        };

        let mut local_results = vec![];
        imdb_lookup(name, year, imdb, query_fn, &mut local_results)?;

        if local_results.is_empty() {
          println!("No matches found for `{}`", display_title(name, year));
        } else if local_results.len() > 1 {
          at_least_one_matched = true;

          println!("Found {} matche(s) for `{}`:", local_results.len(), display_title(name, year));

          sort_results(&mut local_results, sort_by_rating);

          let mut table = create_output_table();

          for res in &local_results {
            let row = create_output_table_row_for_title(res, imdb_url)?;
            table.add_row(row);
          }

          table.printstd();
        } else {
          at_least_one_matched = true;

          results.extend(local_results);
        }
      }
    }
  }

  if !at_least_one {
    println!("No valid directory names");
    return Ok(());
  }

  if !at_least_one_matched {
    println!("None of the directories matched any titles");
    return Ok(());
  }

  sort_results(&mut results, sort_by_rating);

  let mut table = create_output_table();

  for res in &results {
    let row = create_output_table_row_for_title(res, imdb_url)?;
    table.add_row(row);
  }

  table.printstd();

  std::mem::forget(results);
  Ok(())
}

type QueryFn<'a> = fn(db: &'a Imdb, name: &str, year: Option<u16>) -> Res<Vec<Vec<ImdbTitle<'a, 'a>>>>;

fn run(opt: &Opt) -> Res<()> {
  let project = create_project()?;
  let cache_dir = project.cache_dir();
  info!("Cache directory: {}", cache_dir.display());

  fs::create_dir_all(cache_dir)?;
  debug!("Created cache directory");

  const IMDB: &str = "https://www.imdb.com/title/";
  let imdb_url = Url::parse(IMDB)?;

  let start_time = Instant::now();
  let imdb_storage = setup_imdb_storage(cache_dir)?;
  let imdb = Imdb::new(&imdb_storage)?;
  info!("Loaded IMDB database in {}", format_duration(Instant::now().duration_since(start_time)));

  let query_fn = if opt.series {
    Imdb::series_by_title
  } else {
    Imdb::movies_by_title
  };

  let start_time = Instant::now();

  match &opt.command {
    Command::Title { title } => single_title(title, &imdb, query_fn, &imdb_url, opt.sort_by_rating)?,
    Command::Dir { dir } => titles_dir(dir, &imdb, query_fn, &imdb_url, opt.series, opt.sort_by_rating)?,
  }

  info!("IMDB query took {}", format_duration(Instant::now().duration_since(start_time)));

  std::mem::forget(imdb);
  std::mem::forget(imdb_storage);

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

  let logger = env_logger::Builder::new().filter_level(log_level).try_init();
  let have_logger = if let Err(e) = logger {
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
    if have_logger {
      error!("Error: {}", e);
    } else {
      eprintln!("Error: {}", e);
    }
  }

  if have_logger {
    info!("Total time: {}", format_duration(Instant::now().duration_since(start_time)));
  } else {
    eprintln!("Total time: {}", format_duration(Instant::now().duration_since(start_time)));
  }
}
