#![warn(clippy::all)]

mod info;
mod print;
mod search;
mod ui;

use crate::print::{JsonPrinter, OutputFormat, Printer, TablePrinter, YamlPrinter};
use atoi::atoi;
use clap::Parser;
use derive_more::Display;
use directories::ProjectDirs;
use humantime::format_duration;
use indicatif::ProgressBar;
use info::TitleInfo;
use log::{debug, error, log_enabled, warn};
use regex::Regex;
use reqwest::Url;
use search::SearchRes;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tvrank::imdb::{Imdb, ImdbQuery};
use tvrank::Res;
use ui::{create_progress_bar, create_progress_spinner};
use walkdir::WalkDir;

#[derive(Debug, Display)]
#[display(fmt = "{}")]
enum TvRankErr {
  #[display(fmt = "Could not find cache directory")]
  CacheDir,
  #[display(fmt = "Empty set of keywords")]
  NoKeywords,
}

impl TvRankErr {
  fn cache_dir<T>() -> Res<T> {
    Err(Box::new(TvRankErr::CacheDir))
  }

  fn no_keywords<T>() -> Res<T> {
    Err(Box::new(TvRankErr::NoKeywords))
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

#[derive(Debug, clap::Args)]
struct GeneralOpts {
  /// Force updating internal databases
  #[clap(short, long)]
  force_update: bool,

  /// Display colors regardless of the NO_COLOR environment variable
  #[clap(short, long)]
  color: bool,

  /// Verbose output (can be specified multiple times)
  #[clap(short, long, parse(from_occurrences))]
  verbose: u8,
}

#[derive(Debug, clap::Args)]
struct SearchOpts {
  /// Sort by year/rating/title instead of rating/year/title
  #[clap(short = 'y', long)]
  sort_by_year: bool,

  /// Only display the top N results
  #[clap(short, long, name = "N")]
  top: Option<usize>,

  /// Set output format
  #[clap(short, long, arg_enum, default_value = "table")]
  output: OutputFormat,
}

#[derive(Debug, clap::Parser)]
#[clap(about = "Query information about movies and series")]
#[clap(author = "Fred Morcos <fm@fredmorcos.com>")]
struct Opt {
  #[clap(flatten)]
  general_opts: GeneralOpts,

  #[clap(subcommand)]
  command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
  /// Lookup a single title using "KEYWORDS" or "TITLE (YYYY)"
  Title {
    /// Match the given title exactly
    #[clap(short, long)]
    exact: bool,

    /// Search terms, as "KEYWORDS" or "TITLE (YYYY)"
    #[clap(name = "TITLE")]
    title: String,

    #[clap(flatten)]
    general_opts: GeneralOpts,

    #[clap(flatten)]
    search_opts: SearchOpts,
  },

  /// Lookup movie titles from a directory
  MoviesDir {
    /// Directory of movie folders named TITLE (YYYY)
    #[clap(name = "DIR")]
    dir: PathBuf,

    #[clap(flatten)]
    general_opts: GeneralOpts,

    #[clap(flatten)]
    search_opts: SearchOpts,
  },

  /// Lookup series titles from a directory
  SeriesDir {
    /// Directory of series folders named TITLE [(YYYY)]
    #[clap(name = "DIR")]
    dir: PathBuf,

    #[clap(flatten)]
    general_opts: GeneralOpts,

    #[clap(flatten)]
    search_opts: SearchOpts,
  },
}

fn display_title_and_year(title: &str, year: u16) -> String {
  format!("{} ({})", title, year)
}

fn display_keywords(keywords: &[&str]) -> String {
  keywords.join(", ")
}

fn create_keywords_set(title: &str) -> Res<Vec<&str>> {
  debug!("Going to use `{}` as keywords for search query", title);

  let set: HashSet<_> = title.split_whitespace().collect();
  let set: HashSet<_> = if set.is_empty() {
    return TvRankErr::no_keywords();
  } else if set.len() > 1 {
    set.into_iter().filter(|kw| kw.len() > 1).collect()
  } else {
    set
  };

  let keywords: Vec<&str> = set.into_iter().collect();

  if log_enabled!(log::Level::Debug) {
    debug!("Keywords: {}", display_keywords(&keywords));
  }

  Ok(keywords)
}

fn imdb_single_title<'a>(
  title: &str,
  imdb: &'a Imdb,
  imdb_url: &Url,
  sort_by_year: bool,
  top: Option<usize>,
  exact: bool,
  printer: Box<dyn Printer>,
) -> Res<()> {
  let mut movies_results = SearchRes::new_movies(sort_by_year, top);
  let mut series_results = SearchRes::new_series(sort_by_year, top);

  let lc_title;
  let keywords;
  let st: String;
  let search_terms: Option<&str>;

  if let Some((title, year)) = parse_title_and_year(title) {
    lc_title = title.to_lowercase();
    keywords = if exact {
      None
    } else {
      Some(create_keywords_set(&lc_title)?)
    };

    if let Some(keywords) = &keywords {
      movies_results.extend(imdb.by_keywords_and_year(keywords, year, ImdbQuery::Movies));
    } else {
      movies_results.extend(imdb.by_title_and_year(&lc_title, year, ImdbQuery::Movies));
    }

    if let Some(keywords) = &keywords {
      series_results.extend(imdb.by_keywords_and_year(keywords, year, ImdbQuery::Series));
    } else {
      series_results.extend(imdb.by_title_and_year(&lc_title, year, ImdbQuery::Series));
    }

    st = display_title_and_year(title, year);
    search_terms = Some(&st);
  } else {
    lc_title = title.to_lowercase();
    keywords = if exact {
      None
    } else {
      Some(create_keywords_set(&lc_title)?)
    };

    if let Some(keywords) = &keywords {
      movies_results.extend(imdb.by_keywords(keywords, ImdbQuery::Movies));
    } else {
      movies_results.extend(imdb.by_title(&lc_title, ImdbQuery::Movies));
    }

    if let Some(keywords) = &keywords {
      series_results.extend(imdb.by_keywords(keywords, ImdbQuery::Series));
      st = display_keywords(keywords);
      search_terms = Some(&st);
    } else {
      series_results.extend(imdb.by_title(&lc_title, ImdbQuery::Series));
      search_terms = Some(&lc_title);
    }
  }

  printer.print(
    Some(movies_results.top_sorted_results()),
    Some(series_results.top_sorted_results()),
    imdb_url,
    search_terms,
  )?;

  Ok(())
}

fn imdb_movies_dir(
  dir: &Path,
  imdb: &Imdb,
  imdb_url: &Url,
  sort_by_year: bool,
  top: Option<usize>,
  printer: Box<dyn Printer>,
) -> Res<()> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = SearchRes::new_movies(sort_by_year, top);
  let walkdir = WalkDir::new(dir).min_depth(1);

  for entry in walkdir {
    let entry = entry?;

    if entry.file_type().is_dir() {
      let entry_path = entry.path();

      if let Ok(title_info) = TitleInfo::from_path(entry_path) {
        if let Some(result) = imdb.by_id(title_info.imdb().id(), ImdbQuery::Movies) {
          at_least_one_matched = true;
          results.push(result);
          continue;
        } else {
          let id = title_info.imdb().id();
          let path = entry_path.display();
          warn!("Could not find title ID `{id}` for `{path}`, ignoring `tvrank.json` file");
        }
      }

      if let Some(filename) = entry_path.file_name() {
        let filename = filename.to_string_lossy();

        if let Some((title, year)) = parse_title_and_year(&filename) {
          at_least_one = true;

          let mut local_results = SearchRes::new_movies(sort_by_year, top);
          local_results.extend(imdb.by_title_and_year(&title.to_lowercase(), year, ImdbQuery::Movies));

          if local_results.is_empty() || local_results.len() > 1 {
            if local_results.len() > 1 {
              at_least_one_matched = true;
            }

            if matches!(printer.get_format(), OutputFormat::Table) {
              printer.print(
                Some(local_results.top_sorted_results()),
                None,
                imdb_url,
                Some(&display_title_and_year(title, year)),
              )?;
            } else {
              results.extend(local_results);
            }
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
    eprintln!("No valid directory names");
    return Ok(());
  }

  if !at_least_one_matched {
    eprintln!("None of the directories matched any titles");
    return Ok(());
  }

  printer.print(Some(results.top_sorted_results()), None, imdb_url, None)?;

  Ok(())
}

fn imdb_series_dir(
  dir: &Path,
  imdb: &Imdb,
  imdb_url: &Url,
  sort_by_year: bool,
  top: Option<usize>,
  printer: Box<dyn Printer>,
) -> Res<()> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = SearchRes::new_series(sort_by_year, top);
  let walkdir = WalkDir::new(dir).min_depth(1).max_depth(1);

  for entry in walkdir {
    let entry = entry?;

    if entry.file_type().is_dir() {
      let entry_path = entry.path();

      if let Ok(title_info) = TitleInfo::from_path(entry_path) {
        if let Some(result) = imdb.by_id(title_info.imdb().id(), ImdbQuery::Series) {
          at_least_one_matched = true;
          results.push(result);
          continue;
        } else {
          let id = title_info.imdb().id();
          let path = entry_path.display();
          warn!("Could not find title ID `{id}` for `{path}`, ignoring `tvrank.json` file");
        }
      }

      if let Some(filename) = entry_path.file_name() {
        at_least_one = true;

        let filename = filename.to_string_lossy();
        let mut local_results = SearchRes::new_series(sort_by_year, top);

        let search_terms = if let Some((title, year)) = parse_title_and_year(&filename) {
          local_results.extend(imdb.by_title_and_year(&title.to_lowercase(), year, ImdbQuery::Series));
          Cow::from(display_title_and_year(title, year))
        } else {
          local_results.extend(imdb.by_title(&filename.to_lowercase(), ImdbQuery::Series));
          filename
        };

        if local_results.is_empty() || local_results.len() > 1 {
          if local_results.len() > 1 {
            at_least_one_matched = true;
          }

          if matches!(printer.get_format(), OutputFormat::Table) {
            printer.print(None, Some(local_results.top_sorted_results()), imdb_url, Some(&search_terms))?;
          } else {
            results.extend(local_results);
          }
        } else {
          at_least_one_matched = true;
          results.extend(local_results);
        }
      }
    }
  }

  if !at_least_one {
    eprintln!("No valid directory names");
    return Ok(());
  }

  if !at_least_one_matched {
    eprintln!("None of the directories matched any titles");
    return Ok(());
  }

  printer.print(None, Some(results.top_sorted_results()), imdb_url, None)?;

  Ok(())
}

fn run(cmd: &Command, search_opts: &SearchOpts, general_opts: &GeneralOpts) -> Res<()> {
  let project = create_project()?;
  let app_cache_dir = project.cache_dir();
  debug!("Cache directory: {}", app_cache_dir.display());

  fs::create_dir_all(app_cache_dir)?;
  debug!("Created cache directory");

  const IMDB: &str = "https://www.imdb.com/title/";
  let imdb_url = Url::parse(IMDB)?;

  let printer: Box<dyn Printer> = match search_opts.output {
    OutputFormat::Json => Box::new(JsonPrinter {}),
    OutputFormat::Table => Box::new(TablePrinter { color: general_opts.color, top: search_opts.top }),
    OutputFormat::Yaml => Box::new(YamlPrinter {}),
  };

  let start_time = Instant::now();
  let progress_bar: RefCell<Option<ProgressBar>> = RefCell::new(None);

  let imdb = Imdb::new(app_cache_dir, general_opts.force_update, &|content_len: Option<u64>, delta| {
    let mut progress_bar_mut = progress_bar.borrow_mut();
    match &*progress_bar_mut {
      Some(bar) => bar.inc(delta),
      None => {
        let bar = match content_len {
          Some(len) => create_progress_bar("Downloading IMDB databases...".to_string(), len),
          None => create_progress_spinner("Downloading IMDB databases...".to_string()),
        };

        bar.inc(delta);
        *progress_bar_mut = Some(bar);
      }
    }
  })?;

  if let Some(bar) = &*progress_bar.borrow_mut() {
    bar.finish_and_clear();
  }
  debug!("Loaded IMDB database in {}", format_duration(Instant::now().duration_since(start_time)));

  let start_time = Instant::now();

  match cmd {
    Command::Title { exact, title, .. } => {
      imdb_single_title(title, &imdb, &imdb_url, search_opts.sort_by_year, search_opts.top, *exact, printer)?
    }
    Command::MoviesDir { dir, .. } => {
      imdb_movies_dir(dir, &imdb, &imdb_url, search_opts.sort_by_year, search_opts.top, printer)?
    }
    Command::SeriesDir { dir, .. } => {
      imdb_series_dir(dir, &imdb, &imdb_url, search_opts.sort_by_year, search_opts.top, printer)?
    }
  }

  debug!("IMDB query took {}", format_duration(Instant::now().duration_since(start_time)));

  std::mem::forget(imdb);

  Ok(())
}

fn main() {
  let start_time = Instant::now();
  let args = Opt::parse();
  let search_opts = match &args.command {
    Command::Title { search_opts, .. }
    | Command::MoviesDir { search_opts, .. }
    | Command::SeriesDir { search_opts, .. } => search_opts,
  };

  let general_opts = match &args.command {
    Command::Title { general_opts, .. }
    | Command::MoviesDir { general_opts, .. }
    | Command::SeriesDir { general_opts, .. } => GeneralOpts {
      force_update: general_opts.force_update || args.general_opts.force_update,
      color: env::var("NO_COLOR").is_err() || general_opts.color || args.general_opts.color,
      verbose: if general_opts.verbose > 0 {
        general_opts.verbose
      } else {
        args.general_opts.verbose
      },
    },
  };

  let log_level = match general_opts.verbose {
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

  // error!("Error output enabled.");
  // warn!("Warning output enabled.");
  // info!("Info output enabled.");
  // debug!("Debug output enabled.");
  // trace!("Trace output enabled.");

  if let Err(e) = run(&args.command, search_opts, &general_opts) {
    if have_logger {
      error!("Error: {}", e);
    } else {
      eprintln!("Error: {}", e);
    }
  }

  eprintln!("Total time: {}", format_duration(Instant::now().duration_since(start_time)));
}
