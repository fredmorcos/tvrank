#![warn(clippy::all)]

use crate::Res;
use flate2::bufread::GzDecoder;
use log::{debug, info};
use reqwest::{blocking::Client, Url};
use size::Size;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::Path;
use std::time::{Duration, SystemTime};

pub(crate) struct Storage {
  pub basics_db_buf: Vec<u8>,
  pub ratings_db_buf: Vec<u8>,
}

impl Storage {
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

  fn ensure_file(client: &Client, filename: &Path, url: Url, db_name: &str) -> Res<()> {
    let needs_update = {
      let file = Self::file_exists(filename)?;
      Self::file_needs_update(&file)?
    };

    if needs_update {
      info!("IMDB {} DB does not exist or is more than a month old", db_name);
      let mut file = std::fs::File::create(filename)?;
      info!("IMDB {} DB URL: {}", db_name, url);
      let mut res = client.get(url).send()?;
      debug!("Sent request for IMDB {} DB, downloading...", db_name);
      let size = Size::Bytes(res.copy_to(&mut file)?);
      info!(
        "Downloaded IMDB {} DB ({})",
        db_name,
        size.to_string(size::Base::Base10, size::Style::Abbreviated)
      );
    } else {
      info!("IMDB {} DB exists and is less than a month old", db_name);
    }

    Ok(())
  }

  fn decompress_db(filename: &Path, db_name: &str) -> Res<Vec<u8>> {
    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let mut decoder = GzDecoder::new(reader);
    let mut buf = Vec::new();
    info!("Decompressing IMDB {} DB...", db_name);
    let size = Size::Bytes(decoder.read_to_end(&mut buf)?);
    info!(
      "Read IMDB {} DB: {}",
      db_name,
      size.to_string(size::Base::Base10, size::Style::Abbreviated)
    );
    Ok(buf)
  }

  pub(crate) fn load_db_files(app_cache_dir: &Path) -> Res<Self> {
    const IMDB: &str = "https://datasets.imdbws.com/";
    const BASICS: &str = "title.basics.tsv.gz";
    const RATINGS: &str = "title.ratings.tsv.gz";

    let cache_dir = app_cache_dir.join("imdb");
    fs::create_dir_all(&cache_dir)?;
    debug!("Created Imdb cache directory");

    let client = reqwest::blocking::Client::builder().build()?;
    let base_url = Url::parse(IMDB)?;

    let basics_db_file = cache_dir.join(BASICS);
    let basics_db_url = base_url.join(BASICS)?;
    Self::ensure_file(&client, &basics_db_file, basics_db_url, "Basics")?;
    let basics_db_buf = Self::decompress_db(&basics_db_file, "Basics")?;

    let ratings_db_file = cache_dir.join(RATINGS);
    let ratings_db_url = base_url.join(RATINGS)?;
    Self::ensure_file(&client, &ratings_db_file, ratings_db_url, "Ratings")?;
    let ratings_db_buf = Self::decompress_db(&ratings_db_file, "Ratings")?;

    Ok(Self { basics_db_buf, ratings_db_buf })
  }
}
