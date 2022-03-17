#![warn(clippy::all)]

use crate::imdb::db::{Db, Query};
use crate::imdb::progress::Progress;
use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use crate::Res;
use flate2::bufread::GzDecoder;
use fnv::FnvHashSet;
use humantime::format_duration;
use log::{debug, log_enabled};
use parking_lot::{const_mutex, Mutex};
use rayon::prelude::*;
use reqwest::blocking::{Client, Response};
use reqwest::Url;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

/// Struct providing the movies and series databases and the related services
pub struct Service {
  dbs: Vec<Db>,
}

const IMDB: &str = "https://datasets.imdbws.com/";
const RATINGS_FILENAME: &str = "title.ratings.tsv.gz";
const BASICS_FILENAME: &str = "title.basics.tsv.gz";

impl Service {
  /// Returns a Service struct holding movies/series databases
  /// # Arguments
  /// * `cache_dir` - Directory path of the database files
  /// * `force_db_update` - True if the databases should be updated regardless of their age
  /// * `progress_fn` - Function that keeps track of the download progress
  pub fn new(cache_dir: &Path, force_db_update: bool, progress_fn: &dyn Fn(Option<u64>, u64)) -> Res<Self> {
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

  /// Parses titles from the given binary and pushes them into the given vector of titles and the movies/series databases
  /// # Arguments
  /// * `cursor` - Cursor at the binary to read the titles from
  /// * `titles` - Vector to store the titles temporarily before writing to the database
  /// * `db` - Database to store movies or series
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

  /// Parses titles from the given binary and inserts them into the movies/series databases
  /// # Arguments
  /// * `movies_data` - Binary movies data
  /// * `series_data` - Binary series data
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

  /// Returns the file at the given path if it exists, or an Ok Result if it is not found.
  /// Returns an Error if a problem occurs while opening an existing file.
  /// # Arguments
  /// * `path` - Path of the file to be opened
  fn file_exists(path: &Path) -> Res<Option<File>> {
    match File::open(path) {
      Ok(f) => Ok(Some(f)),
      Err(e) => match e.kind() {
        io::ErrorKind::NotFound => Ok(None),
        _ => Err(Box::new(e)),
      },
    }
  }

  /// Determines if the given database needs to be updated. Returns true if the force_db_update parameter is true or if the database
  /// have not been updated for longer than one month.
  /// # Arguments
  /// * `file` - Database file to be checked
  /// * `force_db_update` - True if the database should be updated regardless of its age
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

  /// Sends a GET request to the given URL and returns the response
  /// # Arguments
  /// * `imdb_url` - The base URL to send the GET request to
  /// * `path` - Endpoint path
  fn get_response(imdb_url: &Url, path: &str) -> Res<Response> {
    let url = imdb_url.join(path)?;
    let client = Client::builder().build()?;
    let resp = client.get(url).send()?;
    Ok(resp)
  }

  /// Returns a reader for the given response
  /// # Arguments
  /// * `resp` - Response returned for the GET request
  /// * `progress_fn` - Function to keep track of the download progress
  fn create_downloader(
    resp: Response,
    progress_fn: &dyn Fn(Option<u64>, u64),
  ) -> Res<BufReader<GzDecoder<BufReader<Progress<Response>>>>> {
    let progress = Progress::new(resp, progress_fn);
    let reader = BufReader::new(progress);
    let decoder = GzDecoder::new(reader);
    let reader = BufReader::new(decoder);
    Ok(reader)
  }

  /// Ensures that the movies and series databases exist and are up-to-date. The databases are created if they don't exist, and updated if they
  /// are outdated or if the force_db_update parameter is set to true.
  /// # Arguments
  /// * `movies_db_filename` - Path to the movies database
  /// * `series_db_filename` - Path to the series database
  /// * `force_db_update` - True if the databases should be updated regardless of their age
  /// * `progress_fn` - Function that keeps track of the download progress
  fn ensure_db_files(
    movies_db_filename: &Path,
    series_db_filename: &Path,
    force_db_update: bool,
    progress_fn: &dyn Fn(Option<u64>, u64),
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

      let basics_resp = Self::get_response(&imdb_url, BASICS_FILENAME)?;
      let ratings_resp = Self::get_response(&imdb_url, RATINGS_FILENAME)?;

      match (basics_resp.content_length(), ratings_resp.content_length()) {
        (None, None) | (None, Some(_)) | (Some(_), None) => progress_fn(None, 0),
        (Some(basics_content_len), Some(ratings_content_len)) => {
          progress_fn(Some(basics_content_len + ratings_content_len), 0)
        }
      }

      let basics_downloader = Self::create_downloader(basics_resp, progress_fn)?;
      let ratings_downloader = Self::create_downloader(ratings_resp, progress_fn)?;

      let movies_db_file = File::create(movies_db_filename)?;
      let movies_db_writer = BufWriter::new(movies_db_file);

      let series_db_file = File::create(series_db_filename)?;
      let series_db_writer = BufWriter::new(series_db_file);

      Db::to_binary(ratings_downloader, basics_downloader, movies_db_writer, series_db_writer)?;
    } else {
      debug!("IMDB database exists and is less than a month old");
    }

    Ok(())
  }

  /// Query titles by ID
  /// # Arguments
  /// * `id` - ID of the title to be queried
  /// * `query` - Specifies if movies or series are queried
  pub fn by_id(&self, id: &TitleId, query: Query) -> Option<&Title> {
    let res = self
      .dbs
      .par_iter()
      .map(|db| db.by_id(id, query))
      .filter(|res| res.is_some())
      .flatten()
      .collect::<Vec<_>>();

    if res.is_empty() {
      None
    } else {
      Some(unsafe { res.get_unchecked(0) })
    }
  }

  /// Query titles by title
  /// # Arguments
  /// * `title` - Title to be queried
  /// * `query` - Specifies if movies or series are queried
  pub fn by_title(&self, title: &str, query: Query) -> Vec<&Title> {
    self
      .dbs
      .par_iter()
      .map(|db| db.by_title(title, query).collect::<Vec<_>>())
      .flatten()
      .collect()
  }

  /// Query titles by title and year
  /// # Arguments
  /// * `title` - Title to be queried
  /// * `year` - Release year of the title
  /// * `query` - Specifies if movies or series are queried
  pub fn by_title_and_year(&self, title: &str, year: u16, query: Query) -> Vec<&Title> {
    self
      .dbs
      .par_iter()
      .map(|db| db.by_title_and_year(title, year, query).collect::<Vec<_>>())
      .flatten()
      .collect()
  }

  /// Query titles by keywords
  /// # Arguments
  /// * `keywords` - List of keywords to search in titles
  /// * `query` - Specifies if movies or series are queried
  pub fn by_keywords<'a>(&'a self, keywords: &'a [&str], query: Query) -> FnvHashSet<&'a Title> {
    self
      .dbs
      .par_iter()
      .map(|db| db.by_keywords(keywords, query).collect::<Vec<_>>())
      .flatten()
      .collect()
  }

  /// Query titles by keywords and year
  /// # Arguments
  /// * `keywords` - List of keywords to search in titles
  /// * `year` - Release year of the title
  /// * `query` - Specifies if movies or series are queried
  pub fn by_keywords_and_year<'a>(
    &'a self,
    keywords: &'a [&str],
    year: u16,
    query: Query,
  ) -> FnvHashSet<&'a Title> {
    self
      .dbs
      .par_iter()
      .map(|db| db.by_keywords_and_year(keywords, year, query).collect::<Vec<_>>())
      .flatten()
      .collect()
  }
}
