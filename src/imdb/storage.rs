#![warn(clippy::all)]

use crate::io::write_interactive;
use crate::Res;
use flate2::bufread::GzDecoder;
use indicatif::HumanBytes;
use log::{debug, info};
use reqwest::{Client, Url};
use std::convert::TryInto;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

pub struct Storage {
  pub basics: &'static [u8],
  pub ratings: &'static [u8],
}

impl Storage {
  pub fn new<T1, T2>(
    app_cache_dir: &Path,
    force_update: bool,
    download_cbs: &(impl Fn(&str, Option<u64>) -> T1, impl Fn(&T1, u64), impl Fn(&T1)),
    extract_cbs: &(impl Fn(&str) -> T2, impl Fn(&T2, u64), impl Fn(&T2)),
  ) -> Res<Self> {
    const IMDB: &str = "https://datasets.imdbws.com/";
    const RATINGS_FILENAME: &str = "title.ratings.tsv.gz";
    const BASICS_FILENAME: &str = "title.basics.tsv.gz";

    let cache_dir = app_cache_dir.join("imdb");
    fs::create_dir_all(&cache_dir)?;
    debug!("Created Imdb cache directory");

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let base_url = Url::parse(IMDB)?;

    let (basics, ratings) = runtime.block_on(async {
      let basics = Self::load(
        "IMDB Basics DB",
        &cache_dir,
        &base_url,
        BASICS_FILENAME,
        force_update,
        download_cbs,
        extract_cbs,
      );
      let ratings = Self::load(
        "IMDB Ratings DB",
        &cache_dir,
        &base_url,
        RATINGS_FILENAME,
        force_update,
        download_cbs,
        extract_cbs,
      );

      tokio::join!(basics, ratings)
    });
    let (basics, ratings) = (basics?, ratings?);

    Ok(Self { basics, ratings })
  }

  async fn load<T1, T2>(
    name: &str,
    cache_dir: &Path,
    base_url: &Url,
    filename: &str,
    force_update: bool,
    download_cbs: &(impl Fn(&str, Option<u64>) -> T1, impl Fn(&T1, u64), impl Fn(&T1)),
    extract_cbs: &(impl Fn(&str) -> T2, impl Fn(&T2, u64), impl Fn(&T2)),
  ) -> Res<&'static [u8]> {
    let url = base_url.join(filename)?;
    let filename = cache_dir.join(filename);
    Self::ensure(&filename, url, force_update, name, download_cbs).await?;
    let res = Self::extract(&filename, name, extract_cbs)?;
    Ok(Box::leak(res.into_boxed_slice()))
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

  async fn ensure<T>(
    filename: &Path,
    url: Url,
    force_update: bool,
    name: &str,
    download_cbs: &(impl Fn(&str, Option<u64>) -> T, impl Fn(&T, u64), impl Fn(&T)),
  ) -> Res<()> {
    let needs_update = {
      let file = Self::file_exists(filename)?;
      Self::file_needs_update(&file, force_update)?
    };

    if needs_update {
      if force_update {
        info!("Force-update is enabled, {} is going to re-fetched", name);
      } else {
        info!("{} does not exist or is more than a month old", name);
      }

      let total = Self::download(filename, url, name, download_cbs).await?;
      info!("Downloaded {} ({})", name, HumanBytes(total.try_into()?));
    } else {
      info!("{} exists and is less than a month old", name);
    }

    Ok(())
  }

  async fn download<T>(
    filename: &Path,
    url: Url,
    name: &str,
    download_cbs: &(impl Fn(&str, Option<u64>) -> T, impl Fn(&T, u64), impl Fn(&T)),
  ) -> Res<usize> {
    info!("{} URL: {}", name, url);
    let client = Client::builder().build()?;
    let mut resp = client.get(url).send().await?;
    let mut file = File::create(filename)?;

    let (init, progress, finish) = download_cbs;
    let obj = init(name, resp.content_length());
    let mut total = 0;
    while let Some(chunk) = resp.chunk().await? {
      file.write_all(&chunk)?;
      let delta = chunk.len();
      total += delta;
      progress(&obj, delta.try_into()?);
    }
    finish(&obj);

    Ok(total)
  }

  fn extract<T>(
    filename: &Path,
    name: &str,
    extract_cbs: &(impl Fn(&str) -> T, impl Fn(&T, u64), impl Fn(&T)),
  ) -> Res<Vec<u8>> {
    let file = File::open(&filename)?;
    let reader = BufReader::new(file);
    let mut decoder = GzDecoder::new(reader);
    let mut buf = vec![];

    let (init, progress, finish) = extract_cbs;
    let obj = init(name);
    let total = write_interactive(&mut decoder, &mut buf, |delta| progress(&obj, delta))?;
    finish(&obj);

    info!("Read {}: {}", name, HumanBytes(total.try_into()?));
    Ok(buf)
  }
}
