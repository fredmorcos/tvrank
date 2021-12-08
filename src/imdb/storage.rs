#![warn(clippy::all)]

use crate::io::write_interactive;
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

type DownloadInitFunction<T> = fn(&str, Option<u64>) -> T;
type ExtractInitFunction<T> = fn(&str) -> T;
type ProgressFunction<T> = fn(&T, u64);
type FinishFunction<T> = fn(&T);

pub struct Storage {
  pub basics: &'static [u8],
  pub ratings: &'static [u8],
}

impl Storage {
  pub fn new<T1, T2>(
    app_cache_dir: &Path,
    force_update: bool,
    download_funcs: (DownloadInitFunction<T1>, ProgressFunction<T1>, FinishFunction<T1>),
    extract_funcs: (ExtractInitFunction<T2>, ProgressFunction<T2>, FinishFunction<T2>),
  ) -> Res<Self> {
    const IMDB: &str = "https://datasets.imdbws.com/";
    const BASICS_FILENAME: &str = "title.basics.tsv.gz";
    const RATINGS_FILENAME: &str = "title.ratings.tsv.gz";

    let cache_dir = app_cache_dir.join("imdb");
    fs::create_dir_all(&cache_dir)?;
    debug!("Created Imdb cache directory");

    let client = reqwest::blocking::Client::builder().build()?;
    let base_url = Url::parse(IMDB)?;

    let basics_file = cache_dir.join(BASICS_FILENAME);
    let basics_url = base_url.join(BASICS_FILENAME)?;
    Self::ensure_file(&client, &basics_file, basics_url, force_update, "Basics", download_funcs)?;
    let basics = Self::extract(&basics_file, "Basics", extract_funcs)?;

    let ratings_file = cache_dir.join(RATINGS_FILENAME);
    let ratings_url = base_url.join(RATINGS_FILENAME)?;
    Self::ensure_file(&client, &ratings_file, ratings_url, force_update, "Ratings", download_funcs)?;
    let ratings = Self::extract(&ratings_file, "Ratings", extract_funcs)?;

    Ok(Self { basics: Box::leak(basics.into_boxed_slice()), ratings: Box::leak(ratings.into_boxed_slice()) })
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

  fn file_needs_update(file: &Option<File>, force_update: bool) -> Res<bool> {
    if force_update {
      Ok(true)
    } else if let Some(f) = file {
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

  fn ensure_file<T>(
    client: &Client,
    filename: &Path,
    url: Url,
    force_update: bool,
    db_name: &str,
    download_funcs: (DownloadInitFunction<T>, ProgressFunction<T>, FinishFunction<T>),
  ) -> Res<()> {
    let needs_update = {
      let file = Self::file_exists(filename)?;
      Self::file_needs_update(&file, force_update)?
    };

    if needs_update {
      info!("IMDB {} DB does not exist or is more than a month old", db_name);

      info!("IMDB {} DB URL: {}", db_name, url);
      let mut resp = client.get(url).send()?;
      let mut file = File::create(filename)?;

      let (init, progress, finish) = download_funcs;
      let obj = init(db_name, resp.content_length());
      let total = write_interactive(&mut resp, &mut file, |delta| progress(&obj, delta))?;
      finish(&obj);

      info!("Downloaded IMDB {} DB ({})", db_name, HumanBytes(total.try_into()?));
    } else {
      info!("IMDB {} DB exists and is less than a month old", db_name);
    }

    Ok(())
  }

  fn extract<T>(
    filename: &Path,
    db_name: &str,
    extract_funcs: (ExtractInitFunction<T>, ProgressFunction<T>, FinishFunction<T>),
  ) -> Res<Vec<u8>> {
    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let mut decoder = GzDecoder::new(reader);
    let mut buf = vec![];

    let (init, progress, finish) = extract_funcs;
    let obj = init(db_name);
    let total = write_interactive(&mut decoder, &mut buf, |delta| progress(&obj, delta))?;
    finish(&obj);

    info!("Read IMDB {} DB: {}", db_name, HumanBytes(total.try_into()?));
    Ok(buf)
  }
}
