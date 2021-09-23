#![warn(clippy::all)]

use atoi::atoi;
use derive_more::Display;
use directories::ProjectDirs;
use enum_utils::FromStr;
use flate2::bufread::GzDecoder;
use fnv::FnvHashMap;
use log::{debug, error, info, trace, warn};
use memmap::Mmap;
use regex::Regex;
use reqwest::Url;
use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read};
use std::ops::Index;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, SystemTime};
use structopt::StructOpt;

type Res<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Display, PartialEq, Eq, Hash, Clone, Copy)]
struct TitleId(usize);

#[derive(Debug, Display, FromStr, PartialEq, Eq, Hash, Clone, Copy)]
#[enumeration(rename_all = "camelCase")]
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
  TvPilot,
  RadioSeries,
  RadioEpisode,
}

// impl TitleType {
//   fn is_movie(&self) -> bool {
//     match self {
//       TitleType::Short
//       | TitleType::Video
//       | TitleType::Movie
//       | TitleType::TvMovie
//       | TitleType::TvShort
//       | TitleType::TvSpecial => true,
//       TitleType::VideoGame
//       | TitleType::TvEpisode
//       | TitleType::TvSeries
//       | TitleType::TvMiniSeries
//       | TitleType::TvPilot
//       | TitleType::RadioSeries
//       | TitleType::RadioEpisode => false,
//     }
//   }

//   fn is_series(&self) -> bool {
//     match self {
//       TitleType::Short
//       | TitleType::Video
//       | TitleType::Movie
//       | TitleType::TvMovie
//       | TitleType::TvShort
//       | TitleType::TvSpecial
//       | TitleType::VideoGame => false,
//       TitleType::TvEpisode
//       | TitleType::TvSeries
//       | TitleType::TvMiniSeries
//       | TitleType::TvPilot
//       | TitleType::RadioSeries
//       | TitleType::RadioEpisode => true,
//     }
//   }
// }

#[derive(Debug, Display, FromStr, PartialEq, Eq, Hash, Clone, Copy)]
#[display(fmt = "{}")]
enum Genre {
  Drama = 0,
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
  #[display(fmt = "Reality-TV")]
  #[enumeration(rename = "Reality-TV")]
  RealityTv,
  #[display(fmt = "Sci-Fi")]
  #[enumeration(rename = "Sci-Fi")]
  SciFi,
  #[display(fmt = "Film-Noir")]
  #[enumeration(rename = "Film-Noir")]
  FilmNoir,
  #[display(fmt = "Talk-Show")]
  #[enumeration(rename = "Talk-Show")]
  TalkShow,
  #[display(fmt = "Game-Show")]
  #[enumeration(rename = "Game-Show")]
  GameShow,
}

#[derive(PartialEq, Eq, Default, Clone, Copy)]
struct Genres(u64);

impl Genres {
  fn add_genre(&mut self, genre: Genre) {
    let index = genre as u8;
    self.0 |= 1 << index;
  }
}

impl fmt::Debug for Genres {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    <Self as fmt::Display>::fmt(self, f)
  }
}

impl fmt::Display for Genres {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut first = true;

    for index in 0..u64::BITS {
      let index = index as u8;

      if (self.0 >> index) & 1 == 1 {
        let genre: Genre = unsafe { std::mem::transmute(index) };

        if first {
          write!(f, "{}", genre)?;
          first = false;
        } else {
          write!(f, ", {}", genre)?;
        }
      }
    }

    Ok(())
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct Title {
  title_id: TitleId,
  title_type: TitleType,
  is_adult: bool,
  start_year: Option<u16>,
  end_year: Option<u16>,
  runtime_minutes: Option<u16>,
  genres: Genres,
  average_rating: u8,
  num_votes: u64,
}

impl fmt::Display for Title {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.title_type)?;

    if let Some(year) = self.start_year {
      write!(f, " ({})", year)?;
    }

    write!(f, " [{}]", self.genres)
  }
}

// #[derive(Debug)]
// struct Rating {
//   tconst: u64,
//   average_rating: f32,
//   num_votes: u64,
// }

#[derive(Debug, Display)]
#[display(fmt = "{}")]
enum DbErr {
  #[display(fmt = "ID does not start with `tt` (e.g. ttXXXXXXX)")]
  Id,
  #[display(fmt = "ID does not contain a number (e.g. ttXXXXXXX)")]
  IdNumber,
  #[display(fmt = "Unknown title type")]
  TitleType,
  #[display(fmt = "Bad isAdult marker")]
  BadIsAdult,
  #[display(fmt = "Start year is not a number")]
  StartYear,
  #[display(fmt = "End year is not a number")]
  EndYear,
  #[display(fmt = "Runtime minutes is not a number")]
  RuntimeMinutes,
  #[display(fmt = "ID already exists")]
  IdExists,
  #[display(fmt = "Invalid genre")]
  Genre,
  #[display(fmt = "Unexpected end of file")]
  UnexpectedEof,
}

impl DbErr {
  fn id<T>() -> Res<T> {
    Err(Box::new(DbErr::Id))
  }

  fn bad_is_adult<T>() -> Res<T> {
    Err(Box::new(DbErr::BadIsAdult))
  }

  fn id_exists<T>() -> Res<T> {
    Err(Box::new(DbErr::IdExists))
  }

  fn unexpected_eof<T>() -> Res<T> {
    Err(Box::new(DbErr::UnexpectedEof))
  }
}

impl Error for DbErr {}

type DbByNameAndYear = FnvHashMap<String, FnvHashMap<Option<u16>, Vec<TitleId>>>;

struct Db {
  /// Title information.
  titles: Vec<Option<Title>>,
  /// Map from name to years to titles.
  by_name_and_year: DbByNameAndYear,
}

impl Index<&TitleId> for Db {
  type Output = Option<Title>;

  fn index(&self, index: &TitleId) -> &Self::Output {
    unsafe { self.titles.get_unchecked(index.0) }
  }
}

impl Db {
  const TAB: u8 = b'\t';
  const ZERO: u8 = b'0';
  const ONE: u8 = b'1';
  const COMMA: u8 = b',';
  const NL: u8 = b'\n';

  const NOT_AVAIL: &'static [u8; 2] = b"\\N";

  fn skip_line<R: BufRead>(data: &mut io::Bytes<R>) -> Res<()> {
    for c in data {
      let c = c?;

      if c == Db::NL {
        break;
      }
    }

    Ok(())
  }

  fn parse_cell<R: BufRead>(data: &mut io::Bytes<R>, tok: &mut Vec<u8>) -> Res<()> {
    tok.clear();

    for c in data {
      let c = c?;

      if c == Db::TAB {
        break;
      }

      tok.push(c);
    }

    if tok.is_empty() {
      DbErr::unexpected_eof()
    } else {
      Ok(())
    }
  }

  fn parse_title<R: BufRead>(
    data: &mut io::Bytes<R>,
    tok: &mut Vec<u8>,
    res: &mut String,
  ) -> Res<()> {
    tok.clear();
    res.clear();

    for c in data {
      let c = c?;

      if c == Db::TAB {
        break;
      }

      tok.push(c);
    }

    if tok.is_empty() {
      DbErr::unexpected_eof()
    } else {
      res.push_str(std::str::from_utf8(tok)?);
      Ok(())
    }
  }

  fn parse_is_adult<R: BufRead>(data: &mut io::Bytes<R>) -> Res<bool> {
    let is_adult = match Db::next_byte(data)? {
      Db::ZERO => false,
      Db::ONE => true,
      _ => return DbErr::bad_is_adult(),
    };

    if Db::next_byte(data)? != Db::TAB {
      return DbErr::bad_is_adult();
    }

    Ok(is_adult)
  }

  fn parse_genre<R: BufRead>(
    data: &mut io::Bytes<R>,
    tok: &mut Vec<u8>,
    res: &mut String,
  ) -> Res<bool> {
    tok.clear();
    res.clear();

    let mut finish = false;

    for c in data {
      let c = c?;

      if c == Db::COMMA {
        break;
      } else if c == Db::NL {
        finish = true;
        break;
      }

      tok.push(c);
    }

    if tok.is_empty() || tok == Self::NOT_AVAIL {
      Ok(true)
    } else {
      res.push_str(unsafe { std::str::from_utf8_unchecked(tok) });
      Ok(finish)
    }
  }

  fn parse_genres<R: BufRead>(
    data: &mut io::Bytes<R>,
    tok: &mut Vec<u8>,
    res: &mut String,
  ) -> Res<Genres> {
    let mut genres = Genres::default();

    loop {
      let finish = Self::parse_genre(data, tok, res)?;

      if tok == Self::NOT_AVAIL {
        break;
      }

      let genre = Genre::from_str(res).map_err(|_| DbErr::Genre)?;
      genres.add_genre(genre);

      if finish {
        break;
      }
    }

    Ok(genres)
  }

  fn next_byte<R: BufRead>(data: &mut io::Bytes<R>) -> Res<u8> {
    if let Some(current) = data.next() {
      Ok(current?)
    } else {
      DbErr::unexpected_eof()
    }
  }

  fn new<R: BufRead>(mut data: io::Bytes<R>) -> Res<Self> {
    let mut titles: Vec<Option<Title>> = Vec::new();
    let mut by_name_and_year: DbByNameAndYear = FnvHashMap::default();

    let mut tok = Vec::new();
    let mut res = String::new();

    let _ = Self::skip_line(&mut data)?;

    loop {
      let c = if let Some(c) = data.next() {
        c?
      } else {
        return Ok(Db { titles, by_name_and_year });
      };

      if c != b't' {
        return DbErr::id();
      }

      let c = Db::next_byte(&mut data)?;

      if c != b't' {
        return DbErr::id();
      }

      Db::parse_cell(&mut data, &mut tok)?;
      let id = atoi::<usize>(&tok).ok_or(DbErr::IdNumber)?;

      Db::parse_cell(&mut data, &mut tok)?;
      let title_type = unsafe { std::str::from_utf8_unchecked(&tok) };
      let title_type = TitleType::from_str(title_type).map_err(|_| DbErr::TitleType)?;

      Db::parse_title(&mut data, &mut tok, &mut res)?;
      let ptitle = res.clone();
      Db::parse_title(&mut data, &mut tok, &mut res)?;
      let otitle = if ptitle == res { None } else { Some(res.clone()) };

      let is_adult = Db::parse_is_adult(&mut data)?;

      Db::parse_cell(&mut data, &mut tok)?;
      let start_year = match tok.as_slice() {
        b"\\N" => None,
        start_year => Some(atoi::<u16>(start_year).ok_or(DbErr::StartYear)?),
      };

      Db::parse_cell(&mut data, &mut tok)?;
      let end_year = match tok.as_slice() {
        b"\\N" => None,
        end_year => Some(atoi::<u16>(end_year).ok_or(DbErr::EndYear)?),
      };

      Db::parse_cell(&mut data, &mut tok)?;
      let runtime_minutes = match tok.as_slice() {
        b"\\N" => None,
        runtime_minutes => {
          Some(atoi::<u16>(runtime_minutes).ok_or(DbErr::RuntimeMinutes)?)
        }
      };

      let genres = Db::parse_genres(&mut data, &mut tok, &mut res)?;

      if titles.len() <= id {
        titles.resize_with(id + 1, Default::default);
      } else if unsafe { titles.get_unchecked(id) }.is_some() {
        return DbErr::id_exists();
      }

      let title_id = TitleId(id);

      let title = Title {
        title_id,
        title_type,
        is_adult,
        start_year,
        end_year,
        runtime_minutes,
        genres,
        average_rating: 0,
        num_votes: 0,
      };

      *unsafe { titles.get_unchecked_mut(id) } = Some(title);

      fn update_db(
        db: &mut DbByNameAndYear,
        id: TitleId,
        title: String,
        year: Option<u16>,
      ) {
        db.entry(title)
          .and_modify(|by_year| {
            by_year
              .entry(year)
              .and_modify(|titles| titles.push(id))
              .or_insert_with(|| vec![id]);
          })
          .or_insert_with(|| {
            let mut by_year = FnvHashMap::default();
            by_year.insert(year, vec![id]);
            by_year
          });
      }

      update_db(&mut by_name_and_year, title_id, ptitle, start_year);

      if let Some(otitle) = otitle {
        update_db(&mut by_name_and_year, title_id, otitle, start_year);
      }
    }
  }

  fn lookup(&self, title: &str, year: Option<u16>) -> Option<&Vec<TitleId>> {
    self.by_name_and_year.get(title).and_then(|by_year| by_year.get(&year))
  }
}

trait TvService {
  fn new(cache_dir: &Path) -> Self;
  fn lookup(&self, title: &str, year: u16) -> Res<Option<Vec<Title>>>;
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

fn mmap_file(path: &Path) -> Res<Mmap> {
  let file = File::open(path)?;
  Ok(unsafe { Mmap::map(&file)? })
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

  fn load_basics_db(buf: &[u8]) -> Res<Db> {
    let decoder = BufReader::new(GzDecoder::new(buf));
    info!("Parsing IMDB Basics DB...");
    Db::new(decoder.bytes())
  }

  // fn load_ratings_db(&self) -> Res<Vec<Rating>> {
  //   let mut csv_reader = gz_csv_reader(&self.ratings_db_file)?;
  //   let mut db = Vec::with_capacity(1_000_000);

  //   for title in csv_reader.deserialize() {
  //     let title = title?;
  //     db.push(title);
  //   }

  //   Ok(db)
  // }
}

impl TvService for Imdb {
  fn new(cache_dir: &Path) -> Self {
    let cache_dir = cache_dir.join("imdb");
    let basics_db_file = cache_dir.join(Self::BASICS);
    let ratings_db_file = cache_dir.join(Self::RATINGS);
    Imdb { cache_dir, basics_db_file, ratings_db_file }
  }

  fn lookup(&self, title: &str, year: u16) -> Res<Option<Vec<Title>>> {
    self.ensure_db_files()?;

    // TODO switch out the sample file.
    // let basics_mmap = mmap_file(&self.cache_dir.join("title.basics.small.tsv.gz"))?;
    let basics_mmap = mmap_file(&self.basics_db_file)?;
    let basics_db = Self::load_basics_db(&basics_mmap)?;
    info!("Done loading IMDB Basics DB");

    // let ratings = self.load_ratings_db()?;

    Ok(
      basics_db
        .lookup(title, Some(year))
        .and_then(|ids| ids.iter().map(|id| basics_db[id]).collect()),
    )
  }
}

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

  let imdb = Imdb::new(cache_dir);
  if let Some(titles) = imdb.lookup(title, year)? {
    for title in titles {
      println!("{}", title);
    }
  } else {
    println!("No results");
  }

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
