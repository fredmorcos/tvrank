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

pub struct Storage {
  pub basics_db_buf: &'static [u8],
  pub ratings_db_buf: &'static [u8],
}

impl Storage {
  pub fn new<T1, T2>(
    app_cache_dir: &Path,
    download_init: impl Fn(&str, Option<u64>) -> T1,
    download_during: impl Fn(&T1, u64),
    download_finish: impl Fn(&T1),
    decomp_init: impl Fn(&str) -> T2,
    decomp_during: impl Fn(&T2, u64),
    decomp_finish: impl Fn(&T2),
  ) -> Res<Self> {
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
    Self::ensure_file(
      &client,
      &basics_db_file,
      basics_db_url,
      "Basics",
      &download_init,
      &download_during,
      &download_finish,
    )?;
    let basics_db_buf =
      Self::decompress_db(&basics_db_file, "Basics", &decomp_init, &decomp_during, &decomp_finish)?;

    let ratings_db_file = cache_dir.join(RATINGS);
    let ratings_db_url = base_url.join(RATINGS)?;
    Self::ensure_file(
      &client,
      &ratings_db_file,
      ratings_db_url,
      "Ratings",
      &download_init,
      &download_during,
      &download_finish,
    )?;
    let ratings_db_buf =
      Self::decompress_db(&ratings_db_file, "Ratings", &decomp_init, &decomp_during, &decomp_finish)?;

    Ok(Self {
      basics_db_buf: Box::leak(basics_db_buf.into_boxed_slice()),
      ratings_db_buf: Box::leak(ratings_db_buf.into_boxed_slice()),
    })
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

  fn ensure_file<T>(
    client: &Client,
    filename: &Path,
    url: Url,
    db_name: &str,
    init: impl Fn(&str, Option<u64>) -> T,
    during: impl Fn(&T, u64),
    finish: impl Fn(&T),
  ) -> Res<()> {
    let needs_update = {
      let file = Self::file_exists(filename)?;
      Self::file_needs_update(&file)?
    };

    if needs_update {
      info!("IMDB {} DB does not exist or is more than a month old", db_name);

      info!("IMDB {} DB URL: {}", db_name, url);
      let mut resp = client.get(url).send()?;
      let mut file = File::create(filename)?;

      let obj = init(db_name, resp.content_length());
      let total = write_interactive(&mut resp, &mut file, |delta| during(&obj, delta))?;
      finish(&obj);

      info!("Downloaded IMDB {} DB ({})", db_name, HumanBytes(total.try_into()?));
    } else {
      info!("IMDB {} DB exists and is less than a month old", db_name);
    }

    Ok(())
  }

  fn decompress_db<T>(
    filename: &Path,
    db_name: &str,
    init: impl Fn(&str) -> T,
    during: impl Fn(&T, u64),
    finish: impl Fn(&T),
  ) -> Res<Vec<u8>> {
    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let mut decoder = GzDecoder::new(reader);
    let mut buf = vec![];

    let obj = init(db_name);
    let total = write_interactive(&mut decoder, &mut buf, |delta| during(&obj, delta))?;
    finish(&obj);

    info!("Read IMDB {} DB: {}", db_name, HumanBytes(total.try_into()?));
    Ok(buf)
  }
}
