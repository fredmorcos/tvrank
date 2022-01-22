#![warn(clippy::all)]

use crate::imdb::db::{Db, QueryType};
use crate::imdb::error::Err;
use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use crate::Res;
use flate2::bufread::GzDecoder;
use fnv::FnvHashSet;
use humantime::format_duration;
use log::debug;
use parking_lot::{const_mutex, Mutex};
use reqwest::blocking::Client;
use reqwest::Url;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub struct Service {
  dbs: Vec<Db>,
}

const IMDB: &str = "https://datasets.imdbws.com/";
const RATINGS_FILENAME: &str = "title.ratings.tsv.gz";
const BASICS_FILENAME: &str = "title.basics.tsv.gz";

impl Service {
  pub fn new(cache_dir: &Path, force_db_update: bool, progress_fn: &mut dyn FnMut(u64)) -> Res<Self> {
    // Delete old imdb cache directory.
    let old_cache_dir = cache_dir.join("imdb");
    let _ = fs::remove_dir_all(old_cache_dir);

    let db_filename = cache_dir.join("imdb.tvrankdb");
    Self::ensure_db_file(&db_filename, force_db_update, progress_fn)?;

    let start = Instant::now();
    let data = fs::read(db_filename)?;
    let data = Box::leak(data.into_boxed_slice());
    debug!("Read IMDB database file in {}", format_duration(Instant::now().duration_since(start)));

    let start = Instant::now();
    let svc = Self::from_binary(data)?;
    debug!("Parsed IMDB database in {}", format_duration(Instant::now().duration_since(start)));

    let mut total_movies = 0;
    let mut total_series = 0;
    let mut total_entries = 0;

    for (i, db) in svc.dbs.iter().enumerate() {
      let movies = db.n_movies();
      let series = db.n_series();
      let entries = db.n_entries();

      total_movies += movies;
      total_series += series;
      total_entries += entries;

      debug!("IMDB database (thread {i}) contains {movies} movies and {series} series ({entries} entries)");
    }

    debug!(
      "IMDB database contains {total_movies} movies and {total_series} series ({total_entries} entries)"
    );

    Ok(svc)
  }

  fn from_binary(mut data: &'static [u8]) -> Res<Self> {
    let ncpus = num_cpus::get_physical();
    let dbs = Arc::new(const_mutex(Vec::with_capacity(ncpus)));

    rayon::scope(|scope| {
      let cursor: Arc<Mutex<&mut &[u8]>> = Arc::new(const_mutex(&mut data));

      for _ in 0..ncpus {
        let dbs = Arc::clone(&dbs);
        let cursor = Arc::clone(&cursor);

        scope.spawn(move |_| {
          let mut db = Db::with_capacities(1_900_000 / ncpus, 270_000 / ncpus);
          let mut titles = Vec::with_capacity(100);

          loop {
            let mut cursor = cursor.lock();

            if (*cursor).is_empty() {
              break;
            }

            for _ in 0..100 {
              if (*cursor).is_empty() {
                break;
              }

              let title = match Title::from_binary(&mut cursor) {
                Ok(title) => title,
                Err(e) => panic!("Error parsing title: {}", e),
              };

              titles.push(title);
            }

            drop(cursor);

            for &title in &titles {
              db.store_title(title);
            }

            titles.clear();
          }

          dbs.lock().push(db);
        });
      }
    });

    if let Ok(dbs) = Arc::try_unwrap(dbs) {
      Ok(Self { dbs: dbs.into_inner() })
    } else {
      Err::arc_unwrap()
    }
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

  fn ensure_db_file(db_filename: &Path, force_db_update: bool, progress_fn: &mut dyn FnMut(u64)) -> Res<()> {
    let needs_update = {
      let file = Self::file_exists(db_filename)?;
      Self::file_needs_update(&file, force_db_update)?
    };

    if needs_update {
      if force_db_update {
        debug!("Force-update is enabled, IMDB database is going to be re-fetched and built");
      } else {
        debug!("IMDB database does not exist or is more than a month old, going to fetch and build");
      }

      let imdb_url = Url::parse(IMDB)?;

      let basics_url = imdb_url.join(BASICS_FILENAME)?;
      let basics_client = Client::builder().build()?;
      let basics_resp = basics_client.get(basics_url).send()?;
      let basics_reader = BufReader::new(basics_resp);
      let basics_decoder = GzDecoder::new(basics_reader);
      let basics_reader = BufReader::new(basics_decoder);

      let ratings_url = imdb_url.join(RATINGS_FILENAME)?;
      let ratings_client = Client::builder().build()?;
      let ratings_resp = ratings_client.get(ratings_url).send()?;
      let ratings_reader = BufReader::new(ratings_resp);
      let ratings_decoder = GzDecoder::new(ratings_reader);
      let ratings_reader = BufReader::new(ratings_decoder);

      let db_file = File::create(db_filename)?;
      let db_writer = BufWriter::new(db_file);
      Db::to_binary(ratings_reader, basics_reader, db_writer, progress_fn)?;
    } else {
      debug!("IMDB database exists and is less than a month old");
    }

    Ok(())
  }

  pub fn by_id(&self, id: &TitleId, query_type: QueryType) -> Res<Option<&Title>> {
    let results: FnvHashSet<Option<&Title>> = self
      .dbs
      .iter()
      .map(|db| db.by_id(id, query_type))
      .filter(|title| title.is_some())
      .collect();

    if results.len() > 1 {
      return Err::duplicate_id(id.as_str().to_owned());
    }

    for title in results {
      if title.is_some() {
        return Ok(title);
      }
    }

    Ok(None)
  }

  pub fn by_title(&self, title: &str, query_type: QueryType) -> Res<FnvHashSet<&Title>> {
    Ok(
      self
        .dbs
        .iter()
        .map(move |db| db.by_title(title, query_type))
        .flatten()
        .collect(),
    )
  }

  pub fn by_title_and_year(&self, title: &str, year: u16, query_type: QueryType) -> Res<FnvHashSet<&Title>> {
    Ok(
      self
        .dbs
        .iter()
        .map(move |db| db.by_title_and_year(title, year, query_type))
        .flatten()
        .collect(),
    )
  }

  pub fn by_keywords<'a>(
    &'a self,
    keywords: &'a [&str],
    query_type: QueryType,
  ) -> Res<FnvHashSet<&'a Title>> {
    Ok(
      self
        .dbs
        .iter()
        .map(|db| db.by_keywords(keywords, query_type))
        .flatten()
        .collect(),
    )
  }
}
