#![warn(clippy::all)]

use flate2::bufread::GzDecoder;
use log::{debug, info};
use reqwest::Url;

use super::{db::Db, title::Title};
use crate::Res;
use std::{
  fs::{self, File},
  io::{self, BufReader, Read},
  path::Path,
  time::{Duration, SystemTime},
};

pub struct Service {
  // cache_dir: PathBuf,
  // basics_db_file: PathBuf,
  // ratings_db_file: PathBuf,
  basics_db: Db,
}

fn file_exists(path: &Path) -> Res<Option<File>> {
  match File::open(path) {
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

impl Service {
  const IMDB: &'static str = "https://datasets.imdbws.com/";
  const BASICS: &'static str = "title.basics.tsv.gz";
  const RATINGS: &'static str = "title.ratings.tsv.gz";

  fn ensure_db_files(
    cache_dir: &Path,
    basics_db_file: &Path,
    ratings_db_file: &Path,
  ) -> Res<()> {
    fs::create_dir_all(cache_dir)?;
    debug!("Created Imdb cache directory");

    let client = reqwest::blocking::Client::builder().build()?;
    let imdb = Url::parse(Self::IMDB)?;

    let url = imdb.join(Self::BASICS)?;
    ensure_file(&client, basics_db_file, url, "Imdb Title Basics DB")?;

    let url = imdb.join(Self::RATINGS)?;
    ensure_file(&client, ratings_db_file, url, "Imdb Title Ratings DB")?;

    Ok(())
  }

  fn load_basics_db(buf: BufReader<File>) -> Res<Db> {
    let mut decoder = GzDecoder::new(buf);
    let mut buf = Vec::new();
    info!("Decompressing IMDB Basics DB...");
    let size = decoder.read_to_end(&mut buf)?;
    info!("Read IMDB Basics DB: {} bytes", size);
    info!("Parsing IMDB Basics DB...");
    Db::new(buf.bytes())
  }

  pub fn new(cache_dir: &Path) -> Res<Self> {
    let cache_dir = cache_dir.join("imdb");
    let basics_db_file = cache_dir.join(Self::BASICS);
    let ratings_db_file = cache_dir.join(Self::RATINGS);

    Self::ensure_db_files(&cache_dir, &basics_db_file, &ratings_db_file)?;

    let basics_file = File::open(&basics_db_file)?;
    let basics_db = Self::load_basics_db(BufReader::new(basics_file))?;
    info!("Done loading IMDB Basics DB");

    Ok(Service { basics_db })
  }

  pub fn get_movie(
    &self,
    title: &str,
    year: Option<u16>,
  ) -> Option<impl Iterator<Item = &Title>> {
    self.basics_db.lookup_movie(title, year)
  }
}
