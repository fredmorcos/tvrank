#![warn(clippy::all)]

mod info;
mod search;
mod ui;

use atoi::atoi;
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
use structopt::StructOpt;
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

#[derive(Debug, StructOpt)]
struct Opts {
  /// Force updating internal databases.
  #[structopt(short, long)]
  force_update: bool,

  /// Sort by year/rating/title instead of rating/year/title
  #[structopt(short = "y", long)]
  sort_by_year: bool,

  /// Only display the top N results
  #[structopt(short, long, name = "N")]
  top: Option<usize>,

  /// Display colors regardless of the NO_COLOR environment variable
  #[structopt(short, long)]
  color: bool,

  /// Verbose output (can be specified multiple times)
  #[structopt(short, long, parse(from_occurrences))]
  verbose: u8,
}

#[derive(Debug, StructOpt)]
#[structopt(about = "Query information about movies and series")]
#[structopt(author = "Fred Morcos <fm@fredmorcos.com>")]
struct Opt {
  #[structopt(flatten)]
  opts: Opts,

  #[structopt(subcommand)]
  command: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
  /// Lookup a single title using "KEYWORDS" or "TITLE (YYYY)"
  Title {
    /// Match the given title exactly
    #[structopt(short, long)]
    exact: bool,

    #[structopt(name = "TITLE")]
    title: String,

    #[structopt(flatten)]
    opts: Opts,
  },
  /// Lookup movie titles from a directory
  MoviesDir {
    #[structopt(name = "DIR")]
    dir: PathBuf,

    #[structopt(flatten)]
    opts: Opts,
  },
  /// Lookup series titles from a directory
  SeriesDir {
    #[structopt(name = "DIR")]
    dir: PathBuf,

    #[structopt(flatten)]
    opts: Opts,
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
  color: bool,
  exact: bool,
) -> Res<()> {
  let mut movies_results = SearchRes::new_movies(imdb_url, sort_by_year, top, color);
  let mut series_results = SearchRes::new_series(imdb_url, sort_by_year, top, color);

  if let Some((title, year)) = parse_title_and_year(title) {
    let lc_title = title.to_lowercase();
    let keywords = if exact {
      None
    } else {
      Some(create_keywords_set(&lc_title)?)
    };

    if let Some(keywords) = &keywords {
      movies_results.extend(imdb.by_keywords_and_year(keywords, year, ImdbQuery::Movies));
    } else {
      movies_results.extend(imdb.by_title_and_year(&lc_title, year, ImdbQuery::Movies));
    }

    movies_results.print(Some(&display_title_and_year(title, year)))?;

    if let Some(keywords) = &keywords {
      series_results.extend(imdb.by_keywords_and_year(keywords, year, ImdbQuery::Series));
    } else {
      series_results.extend(imdb.by_title_and_year(&lc_title, year, ImdbQuery::Series));
    }

    series_results.print(Some(&display_title_and_year(title, year)))?;
  } else {
    let lc_title = title.to_lowercase();
    let keywords = if exact {
      None
    } else {
      Some(create_keywords_set(&lc_title)?)
    };

    if let Some(keywords) = &keywords {
      movies_results.extend(imdb.by_keywords(keywords, ImdbQuery::Movies));
      movies_results.print(Some(&display_keywords(keywords)))?;
    } else {
      movies_results.extend(imdb.by_title(&lc_title, ImdbQuery::Movies));
      movies_results.print(Some(&lc_title))?;
    }

    if let Some(keywords) = &keywords {
      series_results.extend(imdb.by_keywords(keywords, ImdbQuery::Series));
      series_results.print(Some(&display_keywords(keywords)))?;
    } else {
      series_results.extend(imdb.by_title(&lc_title, ImdbQuery::Series));
      series_results.print(Some(&lc_title))?;
    }
  }

  Ok(())
}

fn imdb_movies_dir(
  dir: &Path,
  imdb: &Imdb,
  imdb_url: &Url,
  sort_by_year: bool,
  top: Option<usize>,
  color: bool,
) -> Res<()> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = SearchRes::new_movies(imdb_url, sort_by_year, top, color);
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

          let mut local_results = SearchRes::new_movies(imdb_url, sort_by_year, top, color);
          local_results.extend(imdb.by_title_and_year(&title.to_lowercase(), year, ImdbQuery::Movies));

          if local_results.is_empty() || local_results.len() > 1 {
            if local_results.len() > 1 {
              at_least_one_matched = true;
            }

            local_results.print(Some(&display_title_and_year(title, year)))?;
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

  results.print(None)?;
  Ok(())
}

fn imdb_series_dir(
  dir: &Path,
  imdb: &Imdb,
  imdb_url: &Url,
  sort_by_year: bool,
  top: Option<usize>,
  color: bool,
) -> Res<()> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = SearchRes::new_series(imdb_url, sort_by_year, top, color);
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
        let mut local_results = SearchRes::new_series(imdb_url, sort_by_year, top, color);

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

          local_results.print(Some(&search_terms))?;
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

  results.print(None)?;
  Ok(())
}

fn run(cmd: Command, opt: Opts) -> Res<()> {
  let project = create_project()?;
  let app_cache_dir = project.cache_dir();
  debug!("Cache directory: {}", app_cache_dir.display());

  fs::create_dir_all(app_cache_dir)?;
  debug!("Created cache directory");

  const IMDB: &str = "https://www.imdb.com/title/";
  let imdb_url = Url::parse(IMDB)?;

  let start_time = Instant::now();
  let progress_bar: RefCell<Option<ProgressBar>> = RefCell::new(None);

  let imdb = Imdb::new(app_cache_dir, opt.force_update, &|content_len: Option<u64>, delta| {
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
  let color: bool = env::var("NO_COLOR").is_err() || opt.color;

  match cmd {
    Command::Title { exact, title, opts: _ } => {
      imdb_single_title(&title, &imdb, &imdb_url, opt.sort_by_year, opt.top, color, exact)?
    }
    Command::MoviesDir { dir, .. } => {
      imdb_movies_dir(&dir, &imdb, &imdb_url, opt.sort_by_year, opt.top, color)?
    }
    Command::SeriesDir { dir, .. } => {
      imdb_series_dir(&dir, &imdb, &imdb_url, opt.sort_by_year, opt.top, color)?
    }
  }

  debug!("IMDB query took {}", format_duration(Instant::now().duration_since(start_time)));

  std::mem::forget(imdb);

  Ok(())
}

fn main() {
  let start_time = Instant::now();
  let opt = Opt::from_args();
  let opts = match &opt.command {
    Command::Title { opts, .. } => opts,
    Command::MoviesDir { opts, .. } => opts,
    Command::SeriesDir { opts, .. } => opts,
  };

  let merge_opts = Opts {
    force_update: opts.force_update || opt.opts.force_update,
    sort_by_year: opts.sort_by_year || opt.opts.sort_by_year,
    top: if opt.opts.top.is_some() {
      opt.opts.top
    } else {
      opts.top
    },
    color: opts.color || opt.opts.color,
    verbose: if opts.verbose > 0 {
      opts.verbose
    } else {
      opt.opts.verbose
    },
  };

  let log_level = match merge_opts.verbose {
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

  if let Err(e) = run(opt.command, merge_opts) {
    if have_logger {
      error!("Error: {}", e);
    } else {
      eprintln!("Error: {}", e);
    }
  }

  eprintln!("Total time: {}", format_duration(Instant::now().duration_since(start_time)));
}
