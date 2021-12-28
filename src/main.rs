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
use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;
use structopt::StructOpt;
use tvrank::imdb::{Imdb, ImdbQueryType, ImdbStorage, ImdbTitle, ImdbTitleId};
use tvrank::Res;
use ui::{create_progress_bar, create_progress_spinner};
use walkdir::WalkDir;

#[derive(Debug, Display)]
#[display(fmt = "{}")]
enum TvRankErr {
  #[display(fmt = "Could not find cache directory")]
  CacheDir,
  #[display(fmt = "Invalid `tvrank.json` file")]
  InvalidTitleInfo,
}

impl TvRankErr {
  fn cache_dir<T>() -> Res<T> {
    Err(Box::new(TvRankErr::CacheDir))
  }

  fn invalid_title_info<T>() -> Res<T> {
    Err(Box::new(TvRankErr::InvalidTitleInfo))
  }
}

impl Error for TvRankErr {}

fn parse_title_and_year(input: &str) -> Option<(&str, u16)> {
  let regex = match Regex::new(r"^(.+)\s+\((\d{4})\)$") {
    Ok(regex) => regex,
    Err(e) => {
      warn!("Could not parse input `{}` as TITLE (YYYY): {}", input, e);
      return None;
    }
  };

  let captures = match regex.captures(input) {
    Some(captures) => captures,
    None => {
      debug!("Could not parse title and year from `{}`", input);
      return None;
    }
  };

  let title_match = match captures.get(1) {
    Some(title_match) => title_match,
    None => {
      debug!("Could not parse title from `{}`", input);
      return None;
    }
  };

  let year_match = match captures.get(2) {
    Some(year_match) => year_match,
    None => {
      debug!("Could not parse year from `{}`", input);
      return None;
    }
  };

  let year_val = match atoi::<u16>(year_match.as_str().as_bytes()) {
    Some(year_val) => year_val,
    None => {
      warn!("Could not parse year `{}`", year_match.as_str());
      return None;
    }
  };

  Some((title_match.as_str(), year_val))
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

  /// Force updating internal databases.
  #[structopt(short, long)]
  force_update: bool,

  /// Sort by year/rating/title instead of rating/year/title
  #[structopt(short = "y", long)]
  sort_by_year: bool,

  #[structopt(subcommand)]
  command: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
  /// Lookup a single title using "KEYWORDS" or "TITLE (YYYY)"
  Title {
    #[structopt(name = "TITLE")]
    title: String,
  },
  /// Lookup movie titles from a directory
  MoviesDir {
    #[structopt(name = "DIR")]
    dir: PathBuf,
  },
  /// Lookup series titles from a directory
  SeriesDir {
    #[structopt(name = "DIR")]
    dir: PathBuf,
  },
}

fn sort_results(results: &mut [ImdbTitle], by_year: bool) {
  if by_year {
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
  } else {
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

fn create_output_row_for_title(title: &ImdbTitle, imdb_url: &Url) -> Res<Row> {
  static GREEN: Attr = Attr::ForegroundColor(color::GREEN);
  static YELLOW: Attr = Attr::ForegroundColor(color::YELLOW);
  static RED: Attr = Attr::ForegroundColor(color::RED);

  let mut row = Row::new(vec![]);

  row.add_cell(Cell::new(title.primary_title()));

  if title.primary_title() == title.original_title() {
    row.add_cell(Cell::new(""));
  } else {
    row.add_cell(Cell::new(title.original_title()));
  }

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

fn setup_imdb_storage(app_cache_dir: &Path, force_update: bool) -> Res<ImdbStorage> {
  info!("Loading IMDB Databases...");

  // Downloading callbacks.
  let download_init = |name: &str, content_len: Option<u64>| -> ProgressBar {
    let msg = format!("Downloading {}", name);
    let bar = if let Some(file_len) = content_len {
      info!("{} compressed file size is {}", name, HumanBytes(file_len));
      create_progress_bar(msg, file_len)
    } else {
      info!("{} compressed file size is unknown", name);
      create_progress_spinner(msg)
    };

    bar
  };

  let download_progress = |bar: &ProgressBar, delta: u64| {
    bar.inc(delta);
  };

  let download_finish = |bar: &ProgressBar| {
    bar.finish_and_clear();
  };

  // Extraction callbacks.
  let extract_init = |name: &str| -> ProgressBar {
    let msg = format!("Decompressing {}...", name);
    create_progress_spinner(msg)
  };

  let extract_progress = |bar: &ProgressBar, delta: u64| {
    bar.inc(delta);
  };

  let extract_finish = |bar: &ProgressBar| {
    bar.finish_and_clear();
  };

  let imdb_storage = ImdbStorage::new(
    app_cache_dir,
    force_update,
    &(download_init, download_progress, download_finish),
    &(extract_init, extract_progress, extract_finish),
  )?;

  Ok(imdb_storage)
}

fn imdb_by_id<'a>(
  id: &ImdbTitleId,
  imdb: &'a Imdb,
  query_type: ImdbQueryType,
  results: &mut Vec<ImdbTitle<'a, 'a>>,
) -> Res<()> {
  results.extend(imdb.by_id(id, query_type)?);
  Ok(())
}

fn imdb_by_title<'a>(
  title: &str,
  imdb: &'a Imdb,
  query_type: ImdbQueryType,
  results: &mut Vec<ImdbTitle<'a, 'a>>,
) -> Res<()> {
  results.extend(imdb.by_title(&title.to_lowercase(), query_type)?);
  Ok(())
}

fn imdb_by_title_and_year<'a>(
  title: &str,
  year: u16,
  imdb: &'a Imdb,
  query_type: ImdbQueryType,
  results: &mut Vec<ImdbTitle<'a, 'a>>,
) -> Res<()> {
  results.extend(imdb.by_title_and_year(&title.to_lowercase(), year, query_type)?);
  Ok(())
}

fn imdb_by_keywords<'a>(
  _keywords: &'a [&str],
  _imdb: &'a Imdb,
  _query_type: ImdbQueryType,
  _results: &mut Vec<ImdbTitle<'a, 'a>>,
) -> Res<()> {
  unimplemented!("Keyword-based search is not yet implemented");
  // results.extend(imdb.by_keywords(query_type, keywords)?);
  // Ok(())
}

fn display_title_and_year(title: &str, year: u16) -> String {
  format!("{}{}", title, format!(" ({})", year))
}

fn display_keywords(keywords: &[&str]) -> String {
  keywords.join(" ")
}

fn print_search_results(
  search_terms: &str,
  query_type: ImdbQueryType,
  search_results: &[ImdbTitle],
  imdb_url: &Url,
) -> Res<()> {
  if search_results.is_empty() {
    eprintln!("No {} matches found for `{}`", query_type, search_terms);
  } else {
    let matches = if search_results.len() == 1 {
      "match"
    } else {
      "matches"
    };

    eprintln!("Found {} {} {} for `{}`:", search_results.len(), query_type, matches, search_terms);
    let mut table = create_output_table();
    for res in search_results {
      let row = create_output_row_for_title(res, imdb_url)?;
      table.add_row(row);
    }
    table.printstd();
    println!();
  }

  Ok(())
}

fn imdb_single_title<'a>(title: &str, imdb: &'a Imdb, imdb_url: &Url, sort_by_year: bool) -> Res<()> {
  let mut movies_results = vec![];
  let mut series_results = vec![];

  if let Some((title, year)) = parse_title_and_year(title) {
    imdb_by_title_and_year(title, year, imdb, ImdbQueryType::Movies, &mut movies_results)?;
    sort_results(&mut movies_results, sort_by_year);
    print_search_results(
      &display_title_and_year(title, year),
      ImdbQueryType::Movies,
      &movies_results,
      imdb_url,
    )?;

    imdb_by_title_and_year(title, year, imdb, ImdbQueryType::Series, &mut series_results)?;
    sort_results(&mut movies_results, sort_by_year);
    print_search_results(
      &display_title_and_year(title, year),
      ImdbQueryType::Series,
      &series_results,
      imdb_url,
    )?;
  } else {
    warn!("Going to use `{}` as keywords for search query", title);
    let keywords = title
      .split_whitespace()
      .filter(|&keyword| keyword.len() > 2)
      .collect::<Vec<_>>();
    info!("Keywords: {:?}", keywords);

    imdb_by_keywords(&keywords, imdb, ImdbQueryType::Movies, &mut movies_results)?;
    sort_results(&mut movies_results, sort_by_year);
    print_search_results(&display_keywords(&keywords), ImdbQueryType::Movies, &movies_results, imdb_url)?;

    imdb_by_keywords(&keywords, imdb, ImdbQueryType::Series, &mut series_results)?;
    sort_results(&mut series_results, sort_by_year);
    print_search_results(&display_keywords(&keywords), ImdbQueryType::Series, &series_results, imdb_url)?;
  }

  Ok(())
}

#[derive(Deserialize)]
struct TitleInfo {
  imdb: ImdbTitleInfo,
}

#[derive(Deserialize)]
struct ImdbTitleInfo {
  #[serde(deserialize_with = "deserialize_titleid")]
  id: ImdbTitleId<'static>,
}

fn deserialize_titleid<'de, D>(deserializer: D) -> Result<ImdbTitleId<'static>, D::Error>
where
  D: Deserializer<'de>,
{
  let s = Cow::from(String::deserialize(deserializer)?);
  let title_id = ImdbTitleId::try_from(s).map_err(serde::de::Error::custom)?;
  Ok(title_id)
}

fn load_title_info(entry_path: &Path) -> Res<TitleInfo> {
  let title_info_path = entry_path.join("tvrank.json");
  let title_info_file = fs::File::open(&title_info_path)?;
  let title_info_file_reader = BufReader::new(title_info_file);
  let title_info: Result<TitleInfo, _> = serde_json::from_reader(title_info_file_reader);

  let title_info = match title_info {
    Ok(title_info) => title_info,
    Err(err) => {
      warn!("Ignoring info in `{}` due to parse error: {}", title_info_path.display(), err);
      return TvRankErr::invalid_title_info();
    }
  };

  Ok(title_info)
}

fn imdb_movies_dir(dir: &Path, imdb: &Imdb, imdb_url: &Url, sort_by_year: bool) -> Res<()> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = vec![];
  let walkdir = WalkDir::new(dir).min_depth(1);

  for entry in walkdir {
    let entry = entry?;

    if entry.file_type().is_dir() {
      let entry_path = entry.path();

      if let Ok(title_info) = load_title_info(entry_path) {
        let mut local_results = vec![];
        imdb_by_id(&title_info.imdb.id, imdb, ImdbQueryType::Movies, &mut local_results)?;

        if local_results.is_empty() {
          warn!(
            "Could not find title ID `{}` for `{}`, ignoring `tvrank.json` file",
            title_info.imdb.id,
            entry_path.display()
          );
        } else if local_results.len() > 1 {
          warn!(
            "Found {} matches for title ID `{}` for `{}`:",
            local_results.len(),
            title_info.imdb.id,
            entry_path.display()
          );
          warn!("  This should not happen, going to ignore `tvrank.json` file");
          warn!("  but going to print the results for debugging purposes.");
        } else {
          at_least_one_matched = true;
          results.extend(local_results);
          continue;
        }
      }

      if let Some(filename) = entry_path.file_name() {
        let filename = filename.to_string_lossy();

        if let Some((title, year)) = parse_title_and_year(&filename) {
          at_least_one = true;

          let mut local_results = vec![];
          imdb_by_title_and_year(title, year, imdb, ImdbQueryType::Movies, &mut local_results)?;
          sort_results(&mut local_results, sort_by_year);

          if local_results.is_empty() || local_results.len() > 1 {
            if local_results.len() > 1 {
              at_least_one_matched = true;
            }

            print_search_results(
              &display_title_and_year(title, year),
              ImdbQueryType::Movies,
              &local_results,
              imdb_url,
            )?;
          } else {
            at_least_one_matched = true;
            results.extend(local_results);
          }
        } else {
          warn!(
            "Skipping `{}` because `{}` does not follow the TITLE (YYYY) format",
            entry.path().display(),
            filename,
          );

          continue;
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

  sort_results(&mut results, sort_by_year);

  println!("Search results:");
  let mut table = create_output_table();
  for res in &results {
    let row = create_output_row_for_title(res, imdb_url)?;
    table.add_row(row);
  }
  table.printstd();

  Ok(())
}

fn imdb_series_dir(dir: &Path, imdb: &Imdb, imdb_url: &Url, sort_by_year: bool) -> Res<()> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = vec![];
  let walkdir = WalkDir::new(dir).min_depth(1).max_depth(1);

  for entry in walkdir {
    let entry = entry?;

    if entry.file_type().is_dir() {
      let entry_path = entry.path();

      if let Ok(title_info) = load_title_info(entry_path) {
        let mut local_results = vec![];
        imdb_by_id(&title_info.imdb.id, imdb, ImdbQueryType::Series, &mut local_results)?;

        if local_results.is_empty() {
          warn!(
            "Could not find title ID `{}` for `{}`, ignoring `tvrank.json` file",
            title_info.imdb.id,
            entry_path.display()
          );
        } else if local_results.len() > 1 {
          warn!(
            "Found {} matches for title ID `{}` for `{}`:",
            local_results.len(),
            title_info.imdb.id,
            entry_path.display()
          );
          warn!("  This should not happen, going to ignore `tvrank.json` file");
          warn!("  but going to print the results for debugging purposes.");
        } else {
          at_least_one_matched = true;
          results.extend(local_results);
          continue;
        }
      }

      if let Some(filename) = entry_path.file_name() {
        at_least_one = true;

        let filename = filename.to_string_lossy();
        let mut local_results = vec![];

        let search_terms = if let Some((title, year)) = parse_title_and_year(&filename) {
          imdb_by_title_and_year(title, year, imdb, ImdbQueryType::Series, &mut local_results)?;
          sort_results(&mut local_results, sort_by_year);
          Cow::from(display_title_and_year(title, year))
        } else {
          imdb_by_title(&filename, imdb, ImdbQueryType::Series, &mut local_results)?;
          sort_results(&mut local_results, sort_by_year);
          filename
        };

        if local_results.is_empty() || local_results.len() > 1 {
          if local_results.len() > 1 {
            at_least_one_matched = true;
          }

          print_search_results(&search_terms, ImdbQueryType::Series, &local_results, imdb_url)?;
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

  sort_results(&mut results, sort_by_year);

  println!("Search results:");
  let mut table = create_output_table();
  for res in &results {
    let row = create_output_row_for_title(res, imdb_url)?;
    table.add_row(row);
  }
  table.printstd();

  Ok(())
}

fn run(opt: Opt) -> Res<()> {
  let project = create_project()?;
  let app_cache_dir = project.cache_dir();
  info!("Cache directory: {}", app_cache_dir.display());

  fs::create_dir_all(app_cache_dir)?;
  debug!("Created cache directory");

  const IMDB: &str = "https://www.imdb.com/title/";
  let imdb_url = Url::parse(IMDB)?;

  let start_time = Instant::now();
  let imdb_storage = setup_imdb_storage(app_cache_dir, opt.force_update)?;

  let ncpus = rayon::current_num_threads();
  let imdb = Imdb::new(ncpus / 2, &imdb_storage)?;
  eprintln!("Loaded IMDB database in {}", format_duration(Instant::now().duration_since(start_time)));

  let start_time = Instant::now();

  match opt.command {
    Command::Title { title } => imdb_single_title(&title, &imdb, &imdb_url, opt.sort_by_year)?,
    Command::MoviesDir { dir } => imdb_movies_dir(&dir, &imdb, &imdb_url, opt.sort_by_year)?,
    Command::SeriesDir { dir } => imdb_series_dir(&dir, &imdb, &imdb_url, opt.sort_by_year)?,
  }

  eprintln!("IMDB query took {}", format_duration(Instant::now().duration_since(start_time)));

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

  if let Err(e) = run(opt) {
    if have_logger {
      error!("Error: {}", e);
    } else {
      eprintln!("Error: {}", e);
    }
  }

  eprintln!("Total time: {}", format_duration(Instant::now().duration_since(start_time)));
}
