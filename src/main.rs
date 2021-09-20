#![warn(clippy::all)]

use csv::Reader as CsvReader;
use derive_more::Display;
use directories::ProjectDirs;
use flate2::bufread::GzDecoder;
use log::{debug, error, info, trace, warn};
use regex::Regex;
use reqwest::Url;
use serde::{de::Error as SerdeDeserializeError, Deserialize, Deserializer};
use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime};
use structopt::StructOpt;

type Res<T> = Result<T, Box<dyn Error>>;

fn imdb_parse_tconst<'de, D: Deserializer<'de>>(des: D) -> Result<u64, D::Error> {
  let field: &str = Deserialize::deserialize(des)?;

  if field.len() < 2 || &field[0..=1] != "tt" {
    return Err(SerdeDeserializeError::invalid_value(
      serde::de::Unexpected::Str(field),
      &"an ID that starts with `tt` (e.g. ttXXXXX)",
    ));
  }

  match field[2..].parse() {
    Ok(v) => Ok(v),
    Err(_) => Err(SerdeDeserializeError::invalid_value(
      serde::de::Unexpected::Str(field),
      &"a number after the ID's prefix `tt` (e.g. ttXXXXX)",
    )),
  }
}

fn imdb_parse_is_adult<'de, D: Deserializer<'de>>(des: D) -> Result<bool, D::Error> {
  let field: u8 = Deserialize::deserialize(des)?;
  Ok(field != 0)
}

fn imdb_parse_optional_u16<'de, D: Deserializer<'de>>(
  deserializer: D,
) -> Result<Option<u16>, D::Error> {
  let field: &str = Deserialize::deserialize(deserializer)?;

  if field == "\\N" {
    return Ok(None);
  }

  match field.parse() {
    Ok(v) => Ok(Some(v)),
    Err(_) => Err(SerdeDeserializeError::invalid_type(
      serde::de::Unexpected::Str(field),
      &"a number or `\\N`",
    )),
  }
}

#[derive(Debug, Deserialize, Display)]
#[serde(rename_all = "camelCase")]
#[display(fmt = "{}")]
enum TitleType {
  Short,
  Video,
  VideoGame,
  Movie,
  TvMovie,
  TvEpisode,
  TvSeries,
  TvMiniSeries,
  TvShort,
  TvSpecial,
}

#[derive(Debug, Display, PartialEq, Eq, Hash)]
#[display(fmt = "{}")]
enum Genre {
  #[display(fmt = "Reality-TV")]
  RealityTv,
  Drama,
  Documentary,
  Short,
  Animation,
  Comedy,
  Sport,
  Fantasy,
  Horror,
  Romance,
  News,
  Biography,
  Music,
  Musical,
  War,
  Crime,
  Western,
  Family,
  Adventure,
  Action,
  History,
  Mystery,
  Thriller,
  Adult,
  #[display(fmt = "Sci-Fi")]
  SciFi,
  #[display(fmt = "Film-Noir")]
  FilmNoir,
  #[display(fmt = "Talk-Show")]
  TalkShow,
  #[display(fmt = "Game-Show")]
  GameShow,
  #[display(fmt = "N/A")]
  Na,
}

impl FromStr for Genre {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(match s {
      "Reality-TV" => Self::RealityTv,
      "Drama" => Self::Drama,
      "Documentary" => Self::Documentary,
      "Short" => Self::Short,
      "Animation" => Self::Animation,
      "Comedy" => Self::Comedy,
      "Sport" => Self::Sport,
      "Fantasy" => Self::Fantasy,
      "Horror" => Self::Horror,
      "Romance" => Self::Romance,
      "News" => Self::News,
      "Biography" => Self::Biography,
      "Music" => Self::Music,
      "Musical" => Self::Musical,
      "War" => Self::War,
      "Crime" => Self::Crime,
      "Western" => Self::Western,
      "Family" => Self::Family,
      "Adventure" => Self::Adventure,
      "Action" => Self::Action,
      "History" => Self::History,
      "Mystery" => Self::Mystery,
      "Thriller" => Self::Thriller,
      "Adult" => Self::Adult,
      "Sci-Fi" => Self::SciFi,
      "Film-Noir" => Self::FilmNoir,
      "Talk-Show" => Self::TalkShow,
      "Game-Show" => Self::GameShow,
      "\\N" => Self::Na,
      _ => return Err(format!("Unknown genre `{}`", s)),
    })
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Title {
  #[serde(deserialize_with = "imdb_parse_tconst")]
  tconst: u64,
  title_type: TitleType,

  // TODO: Primary and original titles are - most of the time - the same. Perhaps store
  // one of them in an `Option<String>` to avoid searching both.
  primary_title: String,
  original_title: String,

  #[serde(deserialize_with = "imdb_parse_is_adult")]
  is_adult: bool,
  #[serde(deserialize_with = "imdb_parse_optional_u16")]
  start_year: Option<u16>,
  #[serde(deserialize_with = "imdb_parse_optional_u16")]
  end_year: Option<u16>,
  #[serde(deserialize_with = "imdb_parse_optional_u16")]
  runtime_minutes: Option<u16>,
  #[serde(with = "serde_with::StringWithSeparator::<serde_with::CommaSeparator>")]
  genres: HashSet<Genre>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rating {
  #[serde(deserialize_with = "imdb_parse_tconst")]
  tconst: u64,
  average_rating: f32,
  num_votes: u64,
}

trait TvService {
  fn new(cache_dir: &Path) -> Self;
  fn lookup(&self, title: &str, year: &str) -> Res<Vec<&str>>;
}

struct Imdb {
  cache_dir: PathBuf,
  basics_db_file: PathBuf,
  ratings_db_file: PathBuf,
}

fn file_exists(path: &Path) -> Res<Option<File>> {
  match fs::File::open(path) {
    Ok(f) => Ok(Some(f)),
    Err(e) => match e.kind() {
      io::ErrorKind::NotFound => Ok(None),
      _ => Err(Box::new(e)),
    },
  }
}

fn file_needs_update(file: &Option<File>) -> Res<bool> {
  if let Some(f) = file {
    let md = f.metadata()?;
    let modified = md.modified()?;
    let age = match SystemTime::now().duration_since(modified) {
      Ok(duration) => duration,
      Err(_) => return Ok(true),
    };

    // Older than a month.
    Ok(age >= Duration::from_secs(60 * 60 * 24 * 30))
  } else {
    // The file does not exist.
    Ok(true)
  }
}

fn ensure_file(
  client: &reqwest::blocking::Client,
  filename: &Path,
  url: reqwest::Url,
  title: &str,
) -> Res<()> {
  let needs_update = {
    let file = file_exists(filename)?;
    file_needs_update(&file)?
  };

  if needs_update {
    debug!("{} either does not exist or is more than a month old", title);
    let mut file = std::fs::File::create(filename)?;
    debug!("Imdb Title Basics URL: {}", url);
    let mut res = client.get(url).send()?;
    info!("Sent request for {}, downloading...", title);
    let bytes = res.copy_to(&mut file)?;
    debug!("Downloaded {} ({} bytes)", title, bytes);
  } else {
    debug!("{} exists and is less than a month old", title);
  }

  Ok(())
}

fn gz_csv_reader(path: &Path) -> Res<CsvReader<GzDecoder<BufReader<File>>>> {
  let file = File::open(path)?;
  let reader = BufReader::new(file);
  let gz = GzDecoder::new(reader);
  Ok(
    csv::ReaderBuilder::new()
      .flexible(true)
      .trim(csv::Trim::All)
      .delimiter(b'\t')
      .from_reader(gz),
  )
}

impl Imdb {
  const IMDB: &'static str = "https://datasets.imdbws.com/";
  const BASICS: &'static str = "title.basics.tsv.gz";
  const RATINGS: &'static str = "title.ratings.tsv.gz";

  fn ensure_db_files(&self) -> Res<()> {
    fs::create_dir_all(&self.cache_dir)?;
    debug!("Created Imdb cache directory");

    let client = reqwest::blocking::Client::builder().build()?;
    let imdb = Url::parse(Self::IMDB)?;

    let url = imdb.join(Self::BASICS)?;
    ensure_file(&client, &self.basics_db_file, url, "Imdb Title Basics DB")?;

    let url = imdb.join(Self::RATINGS)?;
    ensure_file(&client, &self.ratings_db_file, url, "Imdb Title Ratings DB")?;

    Ok(())
  }

  fn load_basics_db(&self) -> Res<Vec<Title>> {
    let mut csv_reader = gz_csv_reader(&self.basics_db_file)?;
    let mut db = Vec::with_capacity(1_000_000);

    for title in csv_reader.deserialize() {
      let title = title?;
      db.push(title);
    }

    Ok(db)
  }

  fn load_ratings_db(&self) -> Res<Vec<Rating>> {
    let mut csv_reader = gz_csv_reader(&self.ratings_db_file)?;
    let mut db = Vec::with_capacity(1_000_000);

    for title in csv_reader.deserialize() {
      let title = title?;
      db.push(title);
    }

    Ok(db)
  }
}

impl TvService for Imdb {
  fn new(cache_dir: &Path) -> Self {
    let cache_dir = cache_dir.join("imdb");
    let basics_db_file = cache_dir.join(Self::BASICS);
    let ratings_db_file = cache_dir.join(Self::RATINGS);
    Imdb { cache_dir, basics_db_file, ratings_db_file }
  }

  fn lookup(&self, _title: &str, _year: &str) -> Res<Vec<&str>> {
    self.ensure_db_files()?;
    let basics = self.load_basics_db()?;
    let ratings = self.load_ratings_db()?;
    todo!()
  }
}

#[derive(Debug, Display)]
#[display(fmt = "{}")]
enum TvRankErr {
  #[display(fmt = "Invalid title format, must match TITLE (YYYY)")]
  Input,

  #[display(fmt = "Could not read title from input")]
  Title,

  #[display(fmt = "Could not read year from title")]
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
#[structopt(name = "tvrank")]
struct Opt {
  /// Verbose output (can be specified multiple times).
  #[structopt(short, long, parse(from_occurrences))]
  verbose: u8,

  /// Input in the format of TITLE (YYYY).
  #[structopt(name = "INPUT")]
  input: String,
}

fn run(opt: &Opt) -> Res<()> {
  let project = create_project()?;
  let cache_dir = project.cache_dir();
  info!("Cache directory: {}", cache_dir.display());

  fs::create_dir_all(cache_dir)?;
  debug!("Created cache directory");

  let (title, year) = parse_input(&opt.input)?;
  info!("Title: {}", title);
  info!("Year: {}", year);

  let imdb = Imdb::new(cache_dir);
  let res = imdb.lookup(title, year)?;
  debug!("Result = {:#?}", res);

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
