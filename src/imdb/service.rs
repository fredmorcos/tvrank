#![warn(clippy::all)]

use crate::imdb::db::{Db, QueryType};
use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use crate::Res;
use flate2::bufread::GzDecoder;
use fnv::FnvHashSet;
use humantime::format_duration;
use log::{debug, log_enabled};
use parking_lot::{const_mutex, Mutex};
use rayon::prelude::*;
use reqwest::blocking::Client;
use reqwest::Url;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
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

    // Delete old imdb cache file.
    let old_cache_file = cache_dir.join("imdb.tvrankdb");
    let _ = fs::remove_file(old_cache_file);

    let movies_db_filename = cache_dir.join("imdb-movies.tvrankdb");
    let series_db_filename = cache_dir.join("imdb-series.tvrankdb");
    Self::ensure_db_files(&movies_db_filename, &series_db_filename, force_db_update, progress_fn)?;

    let start = Instant::now();
    let movies_data = fs::read(movies_db_filename)?;
    let series_data = fs::read(series_db_filename)?;
    let movies_data = Box::leak(movies_data.into_boxed_slice());
    let series_data = Box::leak(series_data.into_boxed_slice());
    debug!("Read IMDB database in {}", format_duration(Instant::now().duration_since(start)));

    let start = Instant::now();
    let service = Self::from_binary(movies_data, series_data);
    debug!("Parsed IMDB database in {}", format_duration(Instant::now().duration_since(start)));

    if log_enabled!(log::Level::Debug) {
      let mut total_movies = 0;
      let mut total_series = 0;
      let mut total_entries = 0;

      for (i, db) in service.dbs.iter().enumerate() {
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
    }

    Ok(service)
  }

  fn titles_from_binary<const IS_MOVIE: bool>(
    cursor: &Mutex<&mut &'static [u8]>,
    titles: &mut Vec<Title<'static>>,
    db: &mut Db,
  ) {
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

      for &title in titles.iter() {
        if IS_MOVIE {
          db.store_movie(title);
        } else {
          db.store_series(title);
        }
      }

      titles.clear();
    }
  }

  fn from_binary(mut movies_data: &'static [u8], mut series_data: &'static [u8]) -> Self {
    let nthreads = rayon::current_num_threads();
    let dbs = const_mutex(Vec::with_capacity(nthreads));
    let movies_cursor: Mutex<&mut &'static [u8]> = const_mutex(&mut movies_data);
    let series_cursor: Mutex<&mut &'static [u8]> = const_mutex(&mut series_data);

    rayon::scope(|scope| {
      for _ in 0..nthreads {
        let dbs = &dbs;
        let movies_cursor = &movies_cursor;
        let series_cursor = &series_cursor;

        scope.spawn(move |_| {
          let mut db = Db::with_capacities(1_900_000 / nthreads, 270_000 / nthreads);
          let mut titles = Vec::with_capacity(100);
          Self::titles_from_binary::<true>(movies_cursor, &mut titles, &mut db);
          Self::titles_from_binary::<false>(series_cursor, &mut titles, &mut db);
          dbs.lock().push(db);
        });
      }
    });

    Self { dbs: dbs.into_inner() }
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

  fn file_needs_update(file: &Option<File>, force_db_update: bool) -> Res<bool> {
    if force_db_update {
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

  fn ensure_db_files(
    movies_db_filename: &Path,
    series_db_filename: &Path,
    force_db_update: bool,
    progress_fn: &mut dyn FnMut(u64),
  ) -> Res<()> {
    let needs_update = {
      let movies_db_file = Self::file_exists(movies_db_filename)?;
      let series_db_file = Self::file_exists(series_db_filename)?;
      Self::file_needs_update(&movies_db_file, force_db_update)?
        || Self::file_needs_update(&series_db_file, force_db_update)?
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

      let movies_db_file = File::create(movies_db_filename)?;
      let movies_db_writer = BufWriter::new(movies_db_file);

      let series_db_file = File::create(series_db_filename)?;
      let series_db_writer = BufWriter::new(series_db_file);

      Db::to_binary(ratings_reader, basics_reader, movies_db_writer, series_db_writer, progress_fn)?;
    } else {
      debug!("IMDB database exists and is less than a month old");
    }

    Ok(())
  }

  pub fn by_id(&self, id: &TitleId, query_type: QueryType) -> Option<&Title> {
    let res = self
      .dbs
      .par_iter()
      .map(|db| db.by_id(id, query_type))
      .filter(|res| res.is_some())
      .flatten()
      .collect::<Vec<_>>();

    if res.is_empty() {
      None
    } else {
      Some(unsafe { res.get_unchecked(0) })
    }
  }

  pub fn by_title(&self, title: &str, query_type: QueryType) -> Vec<&Title> {
    self
      .dbs
      .par_iter()
      .map(|db| db.by_title(title, query_type).collect::<Vec<_>>())
      .flatten()
      .collect()
  }

  pub fn by_title_and_year(&self, title: &str, year: u16, query_type: QueryType) -> Vec<&Title> {
    self
      .dbs
      .par_iter()
      .map(|db| db.by_title_and_year(title, year, query_type).collect::<Vec<_>>())
      .flatten()
      .collect()
  }

  pub fn by_keywords<'a>(&'a self, keywords: &'a [&str], query_type: QueryType) -> FnvHashSet<&'a Title> {
    self
      .dbs
      .par_iter()
      .map(|db| db.by_keywords(keywords, query_type).collect::<Vec<_>>())
      .flatten()
      .collect()
  }
}
