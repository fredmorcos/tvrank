#![warn(clippy::all)]

use crate::io::write_interactive;
use crate::progressbar::{create_progress_bar, create_progress_spinner};
use crate::Res;
use flate2::bufread::GzDecoder;
use indicatif::HumanBytes;
use log::{debug, info};
use reqwest::{blocking::Client, Url};
use std::convert::TryInto;
use std::fs::{self, File};
use std::io::{self, BufReader};
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
      info!("IMDB {} DB URL: {}", db_name, url);
      let mut resp = client.get(url).send()?;

      info!("IMDB {} DB does not exist or is more than a month old", db_name);
      let mut file = File::create(filename)?;

      let msg = format!("Downloading IMDB {} DB", db_name);
      let bar = if let Some(file_length) = resp.content_length() {
        info!("IMDB {} DB compressed file size is {}", db_name, HumanBytes(file_length));
        create_progress_bar(msg, file_length)
      } else {
        info!("IMDB {} DB compressed file size is unknown", db_name);
        create_progress_spinner(msg)
      };

      let total = write_interactive(&mut resp, &mut file, |delta| {
        bar.inc(delta.try_into()?);
        Ok(())
      })?;

      bar.finish_and_clear();

      info!("Downloaded IMDB {} DB ({})", db_name, HumanBytes(total.try_into()?));
    } else {
      info!("IMDB {} DB exists and is less than a month old", db_name);
    }

    Ok(())
  }

  fn decompress_db(filename: &Path, db_name: &str) -> Res<Vec<u8>> {
    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let mut decoder = GzDecoder::new(reader);
    let mut buf = vec![];

    let msg = format!("Decompressing IMDB {} DB...", db_name);
    let spinner = create_progress_spinner(msg);

    let total = write_interactive(&mut decoder, &mut buf, |delta| {
      spinner.inc(delta.try_into()?);
      Ok(())
    })?;

    spinner.finish_and_clear();

    info!("Read IMDB {} DB: {}", db_name, HumanBytes(total.try_into()?));
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
