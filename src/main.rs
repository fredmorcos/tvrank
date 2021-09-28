#![warn(clippy::all)]

use atoi::atoi;
use derive_more::Display;
use directories::ProjectDirs;
use log::{debug, error, info, trace, warn};
use regex::Regex;
use std::error::Error;
use std::fs;
use tvrank::imdb::error::DbErr;
use tvrank::Res;
// use std::path::{Path, PathBuf};
use structopt::StructOpt;
use tvrank::imdb::service::Imdb;

// impl TvService for Imdb {
//   fn new(cache_dir: &Path) -> Res<Self> {
//     let cache_dir = cache_dir.join("imdb");
//     let basics_db_file = cache_dir.join(Self::BASICS);
//     let ratings_db_file = cache_dir.join(Self::RATINGS);

//     Self::ensure_db_files(&cache_dir, &basics_db_file, &ratings_db_file)?;

//     // TODO switch out the sample file.
//     // let basics_mmap = mmap_file(&self.cache_dir.join("title.basics.small.tsv.gz"))?;
//     // let basics_mmap = mmap_file(&self.basics_db_file)?;
//     let basics_file = File::open(&basics_db_file)?;
//     // let mut buf = Vec::with_capacity(basics_file.metadata()?.len() as usize);
//     // basics_file.read_to_end(&mut buf)?;
//     let basics_db = Self::load_basics_db(BufReader::new(basics_file))?;
//     info!("Done loading IMDB Basics DB");

//     // let ratings = self.load_ratings_db()?;

//     // Ok(Imdb { cache_dir, basics_db_file, ratings_db_file, basics_db })
//     Ok(Imdb { basics_db })
//   }

//   fn lookup(&self, title: &str, year: u16) -> Res<Option<Vec<&Title>>> {
//     Ok(
//       self
//         .basics_db
//         .lookup(title, Some(year))
//         .map(|cookies| cookies.iter().map(|cookie| &self.basics_db[cookie]).collect()),
//     )
//   }
// }

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

// fn parse_input_movie(input: &str) -> Res<(&str, &str)> {
//   // let (title, captures) = parse_input(input, r"^(.+)\s+\((\d{4})\)$");
//   todo!()
// }

fn parse_input(input: &str) -> Res<(&str, &str)> {
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
  let (title, year) = parse_input(input)?;
  let year = atoi::<u16>(year.as_bytes()).ok_or(DbErr::IdNumber)?;
  info!("Title: {}", title);
  info!("Year: {}", year);

  let imdb = Imdb::new(cache_dir)?;
  if let Some(titles) = imdb.get_movie(title, Some(year)) {
    for title in titles {
      println!("{}", title);
    }
  } else {
    println!("No results");
  }

  std::mem::forget(imdb);

  Ok(())
}

fn main() {
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
}
