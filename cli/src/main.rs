#![warn(clippy::all)]

mod print;
mod search;
mod ui;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, io};

use crate::print::{JsonPrinter, OutputFormat, Printer, TablePrinter, YamlPrinter};
use crate::search::SearchRes;
use crate::ui::{create_progress_bar, create_progress_spinner};

use tvrank::imdb::{Imdb, ImdbError, ImdbQuery, ImdbTitleId, ImdbTitleIdError};
use tvrank::title_info::TitleInfo;
use tvrank::utils::search::{SearchString, SearchStringError};

use atoi::atoi;
use clap::Parser;
use directories::ProjectDirs;
use humantime::format_duration;
use indicatif::ProgressBar;
use log::{debug, error, log_enabled, warn};
use regex::Regex;
use reqwest::Url;
use walkdir::WalkDir;

#[derive(Debug, thiserror::Error)]
#[error("TVrank error")]
enum Error {
  #[error("Empty set of keywords")]
  EmptyKeywords,
  #[error("Invalid search string `{0}`")]
  SearchString(#[from] SearchStringError),
  #[error("Output error: {0}")]
  Print(#[from] print::Error),
  #[error("Directory error: {0}")]
  Walkdir(#[from] walkdir::Error),
  #[error("IMDB title ID error: {0}")]
  ImdbTitleId(#[from] ImdbTitleIdError),
  #[error("`{}` is not a directory", .0.display())]
  NotDir(PathBuf),
  #[error("Unknown IMDB ID `{0}`")]
  UnknownImdbId(String),
  #[error("IO error: {0}")]
  Io(#[from] io::Error),
  #[error("Error writing title information file: {0}")]
  Json(#[from] serde_json::Error),
  #[error("Cannot find cache directory")]
  CacheDir,
  #[error("URL parse error: {0}")]
  Url(#[from] url::ParseError),
  #[error("IMDB service error: {0}")]
  Imdb(#[from] ImdbError),
}

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

#[derive(Debug, clap::Args)]
struct GeneralOpts {
  /// Force updating internal databases
  #[clap(short, long)]
  force_update: bool,

  /// Display colors even when the NO_COLOR environment variable is set
  #[clap(short, long)]
  color: bool,

  /// Verbose output (can be specified multiple times)
  #[clap(short, long, action = clap::ArgAction::Count)]
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
  #[clap(short, long, value_enum, default_value = "table")]
  output: OutputFormat,
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Opt {
  #[clap(flatten)]
  general_opts: GeneralOpts,

  #[clap(subcommand)]
  command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
  /// Lookup a single title using "KEYWORDS" or "TITLE (YYYY)"
  Search {
    /// Search terms, as "KEYWORDS" or "TITLE (YYYY)"
    #[clap(name = "TITLE")]
    title: String,

    /// Match the given title exactly
    #[clap(short, long)]
    exact: bool,

    #[clap(flatten)]
    general_opts: GeneralOpts,

    #[clap(flatten)]
    search_opts: SearchOpts,
  },

  /// Lookup movie titles from a directory
  ScanMovies {
    /// Directory of movie folders named "TITLE (YYYY)"
    #[clap(name = "DIR")]
    dir: PathBuf,

    #[clap(flatten)]
    general_opts: GeneralOpts,

    #[clap(flatten)]
    search_opts: SearchOpts,
  },

  /// Lookup series titles from a directory
  ScanSeries {
    /// Directory of series folders named "TITLE [(YYYY)]"
    #[clap(name = "DIR")]
    dir: PathBuf,

    #[clap(flatten)]
    general_opts: GeneralOpts,

    #[clap(flatten)]
    search_opts: SearchOpts,
  },

  /// Force DIR to use IMDB-ID in case of ambiguity
  Mark {
    /// Directory of title in "TITLE [(YYYY)]"
    #[clap(name = "DIR")]
    dir: PathBuf,

    /// The unique IMDB ID ("ttXXXXX" which can be found in the URL)
    #[clap(name = "IMDB-ID")]
    id: String,

    /// Force overwriting of the title information (tvrank.json) file
    #[clap(short, long)]
    force: bool,

    #[clap(flatten)]
    general_opts: GeneralOpts,
  },
}

fn display_title_and_year(title: &str, year: u16) -> String {
  format!("{title} ({year})")
}

fn display_keywords(keywords: &[SearchString]) -> String {
  keywords.iter().map(|kw| kw.as_str()).collect::<Vec<_>>().join(", ")
}

fn create_keywords_set(title: &str) -> Result<Vec<SearchString>, Error> {
  debug!("Going to use `{}` as keywords for search query", title);

  let set: HashSet<_> = title.split_whitespace().collect();
  let set: HashSet<_> = if set.is_empty() {
    return Err(Error::EmptyKeywords);
  } else if set.len() > 1 {
    set.into_iter().filter(|kw| kw.len() > 1).collect()
  } else {
    set
  };

  let keywords = set
    .into_iter()
    .map(SearchString::try_from)
    .collect::<Result<Vec<_>, SearchStringError>>()?;

  if log_enabled!(log::Level::Debug) {
    debug!("Keywords: {}", display_keywords(&keywords));
  }

  Ok(keywords)
}

fn imdb_title(
  title: &str,
  imdb: &Imdb,
  imdb_url: &Url,
  search_opts: &SearchOpts,
  exact: bool,
  printer: Box<dyn Printer<Error = crate::print::Error>>,
) -> Result<(), Error> {
  let mut movies_results = SearchRes::new(search_opts.sort_by_year, search_opts.top);
  let mut series_results = SearchRes::new(search_opts.sort_by_year, search_opts.top);

  let search_terms = if let Some((title, year)) = parse_title_and_year(title) {
    if exact {
      let search_string = SearchString::try_from(title)?;
      movies_results.extend(imdb.by_title_and_year(&search_string, year, ImdbQuery::Movies));
      series_results.extend(imdb.by_title_and_year(&search_string, year, ImdbQuery::Series));
    } else {
      let keywords = create_keywords_set(title)?;
      movies_results.extend(imdb.by_keywords_and_year(&keywords, year, ImdbQuery::Movies));
      series_results.extend(imdb.by_keywords_and_year(&keywords, year, ImdbQuery::Series));
    }

    Some(display_title_and_year(title, year))
  } else if exact {
    let search_string = SearchString::try_from(title)?;
    movies_results.extend(imdb.by_title(&search_string, ImdbQuery::Movies));
    series_results.extend(imdb.by_title(&search_string, ImdbQuery::Series));
    Some(search_string.into())
  } else {
    let keywords = create_keywords_set(title)?;
    movies_results.extend(imdb.by_keywords(&keywords, ImdbQuery::Movies));
    series_results.extend(imdb.by_keywords(&keywords, ImdbQuery::Series));
    Some(display_keywords(&keywords))
  };

  printer.print(Some(movies_results), Some(series_results), imdb_url, search_terms.as_deref())?;

  Ok(())
}

fn imdb_movies_dir(
  dir: &Path,
  imdb: &Imdb,
  imdb_url: &Url,
  search_opts: &SearchOpts,
  printer: Box<dyn Printer<Error = crate::print::Error>>,
) -> Result<(), Error> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = SearchRes::new(search_opts.sort_by_year, search_opts.top);
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

          let mut local_results = SearchRes::new(search_opts.sort_by_year, None);
          let search_string = SearchString::try_from(title)?;
          local_results.extend(imdb.by_title_and_year(&search_string, year, ImdbQuery::Movies));

          if local_results.is_empty() || local_results.len() > 1 {
            if local_results.len() > 1 {
              at_least_one_matched = true;
            }

            if matches!(printer.get_format(), OutputFormat::Table) {
              printer.print(
                Some(local_results),
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

  printer.print(Some(results), None, imdb_url, None)?;

  Ok(())
}

fn imdb_mark(dir: &Path, id: &str, imdb: &Imdb, force: bool) -> Result<(), Error> {
  // TODO: Check if the directory follows the naming convention.
  // TODO: Check if the imdb id matches the title and year of the directory name.

  let title_id = ImdbTitleId::try_from(id)?;

  if !dir.is_dir() {
    return Err(Error::NotDir(dir.to_owned()));
  }

  if imdb
    .by_id(&title_id, ImdbQuery::Movies)
    .or_else(|| imdb.by_id(&title_id, ImdbQuery::Series))
    .is_none()
  {
    return Err(Error::UnknownImdbId(id.to_owned()));
  }

  let title_info = TitleInfo::new(title_id);

  let title_info_path = dir.join("tvrank.json");
  let mut file = OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .create_new(!force)
    .open(title_info_path)?;
  file.write_all(serde_json::to_string_pretty(&title_info)?.as_bytes())?;

  Ok(())
}

fn imdb_series_dir(
  dir: &Path,
  imdb: &Imdb,
  imdb_url: &Url,
  search_opts: &SearchOpts,
  printer: Box<dyn Printer<Error = crate::print::Error>>,
) -> Result<(), Error> {
  let mut at_least_one = false;
  let mut at_least_one_matched = false;
  let mut results = SearchRes::new(search_opts.sort_by_year, search_opts.top);
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
        let mut local_results = SearchRes::new(search_opts.sort_by_year, None);

        let search_terms = if let Some((title, year)) = parse_title_and_year(&filename) {
          let search_string = SearchString::try_from(title)?;
          local_results.extend(imdb.by_title_and_year(&search_string, year, ImdbQuery::Series));
          Cow::from(display_title_and_year(title, year))
        } else {
          local_results.extend(imdb.by_title(&SearchString::try_from(filename.as_ref())?, ImdbQuery::Series));
          filename
        };

        if local_results.is_empty() || local_results.len() > 1 {
          if local_results.len() > 1 {
            at_least_one_matched = true;
          }

          if matches!(printer.get_format(), OutputFormat::Table) {
            printer.print(None, Some(local_results), imdb_url, Some(&search_terms))?;
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

  printer.print(None, Some(results), imdb_url, None)?;

  Ok(())
}

fn create_project() -> Result<ProjectDirs, Error> {
  let prj = ProjectDirs::from("com.fredmorcos", "Fred Morcos", "tvrank");
  if let Some(prj) = prj {
    Ok(prj)
  } else {
    Err(Error::CacheDir)
  }
}

fn create_cache_dir(project: &ProjectDirs) -> Result<&Path, Error> {
  let app_cache_dir = project.cache_dir();
  fs::create_dir_all(app_cache_dir)?;
  debug!("Cache directory: {}", app_cache_dir.display());
  Ok(app_cache_dir)
}

fn get_imdb_url() -> Result<Url, Error> {
  const IMDB: &str = "https://www.imdb.com/title/";
  let imdb_url = Url::parse(IMDB)?;
  Ok(imdb_url)
}

fn create_output_printer(
  output_format: &OutputFormat,
  general_opts: &GeneralOpts,
) -> Box<dyn Printer<Error = crate::print::Error>> {
  match output_format {
    OutputFormat::Json => Box::new(JsonPrinter::new()),
    OutputFormat::Table => Box::new(TablePrinter::new(general_opts.color)),
    OutputFormat::Yaml => Box::new(YamlPrinter::new()),
  }
}

fn create_imdb_service(app_cache_dir: &Path, force_update: bool) -> Result<Imdb, Error> {
  let start_time = Instant::now();
  let progress_bar: RefCell<Option<ProgressBar>> = RefCell::new(None);
  let imdb = Imdb::new(app_cache_dir, force_update, |content_len: Option<u64>, delta| {
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
  Ok(imdb)
}

fn is_no_color_env_set() -> bool {
  match env::var("NO_COLOR") {
    Ok(val) => val != "0",
    Err(_) => false,
  }
}

fn merge_general_opts(locals: GeneralOpts, globals: GeneralOpts) -> GeneralOpts {
  GeneralOpts {
    force_update: locals.force_update || globals.force_update,
    color: !is_no_color_env_set() || locals.color || globals.color,
    verbose: if locals.verbose > 0 {
      locals.verbose
    } else {
      globals.verbose
    },
  }
}

fn get_log_level(verbose: u8) -> log::LevelFilter {
  match verbose {
    0 => log::LevelFilter::Off,
    1 => log::LevelFilter::Error,
    2 => log::LevelFilter::Warn,
    3 => log::LevelFilter::Info,
    4 => log::LevelFilter::Debug,
    _ => log::LevelFilter::Trace,
  }
}

macro_rules! fail {
  ($logger:expr, $e:expr) => {{
    fail!($logger, $e => {})
  }};

  ($logger:expr, $e:expr => $exec:block) => {
    match $e {
      Ok(v) => v,
      Err(e) => {
        let logger: bool = $logger;
        if logger {
          error!("Error: {e}");
        } else {
          eprintln!("Error: {e}");
        }
        $exec;
        std::process::exit(1);
      }
    }
  };
}

struct Context {
  general_opts: GeneralOpts,
  have_logger: bool,
  imdb_url: Url,
  service: Imdb,
}

impl Context {
  fn new(locals: GeneralOpts, globals: GeneralOpts) -> Self {
    let general_opts = merge_general_opts(locals, globals);
    let log_level = get_log_level(general_opts.verbose);
    let logger = env_logger::Builder::new().filter_level(log_level).try_init();
    if let Err(e) = &logger {
      eprintln!("Error initializing logger: {e}");
    }
    let have_logger = logger.is_err();

    // error!("Error output enabled.");
    // warn!("Warning output enabled.");
    // info!("Info output enabled.");
    // debug!("Debug output enabled.");
    // trace!("Trace output enabled.");

    let project = fail!(have_logger, create_project());
    let app_cache_dir = fail!(have_logger, create_cache_dir(&project));
    let imdb_url = fail!(have_logger, get_imdb_url());
    let service = fail!(have_logger, create_imdb_service(app_cache_dir, general_opts.force_update));

    Self { general_opts, have_logger, imdb_url, service }
  }

  fn destroy(self) {
    std::mem::forget(self)
  }
}

fn main() {
  let start_time = Instant::now();
  let args = Opt::parse();

  match args.command {
    Command::Search { title, exact, general_opts, search_opts } => {
      let context = Context::new(general_opts, args.general_opts);
      let printer = create_output_printer(&search_opts.output, &context.general_opts);
      let start_time = Instant::now();
      fail!(context.have_logger, imdb_title(&title, &context.service, &context.imdb_url, &search_opts, exact, printer) => {
        context.destroy();
      });
      debug!("IMDB query took {}", format_duration(Instant::now().duration_since(start_time)));
      context.destroy();
    }
    Command::ScanMovies { dir, general_opts, search_opts } => {
      let context = Context::new(general_opts, args.general_opts);
      let printer = create_output_printer(&search_opts.output, &context.general_opts);
      let start_time = Instant::now();
      fail!(context.have_logger, imdb_movies_dir(&dir, &context.service, &context.imdb_url, &search_opts, printer) => {
        context.destroy();
      });
      debug!("IMDB query took {}", format_duration(Instant::now().duration_since(start_time)));
      context.destroy();
    }
    Command::ScanSeries { dir, general_opts, search_opts } => {
      let context = Context::new(general_opts, args.general_opts);
      let printer = create_output_printer(&search_opts.output, &context.general_opts);
      let start_time = Instant::now();
      fail!(context.have_logger, imdb_series_dir(&dir, &context.service, &context.imdb_url, &search_opts, printer) => {
        context.destroy();
      });
      debug!("IMDB query took {}", format_duration(Instant::now().duration_since(start_time)));
      context.destroy();
    }
    Command::Mark { dir, id, general_opts, force } => {
      let context = Context::new(general_opts, args.general_opts);
      let start_time = Instant::now();
      fail!(context.have_logger, imdb_mark(&dir, &id, &context.service, force) => {
        context.destroy();
      });
      debug!("IMDB query took {}", format_duration(Instant::now().duration_since(start_time)));
      context.destroy();
    }
  }

  eprintln!("Total time: {}", format_duration(Instant::now().duration_since(start_time)));
}
