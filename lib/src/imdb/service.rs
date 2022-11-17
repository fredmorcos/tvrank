#![warn(clippy::all)]

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use crate::imdb::db::Query;
use crate::imdb::db_binary::ServiceDbFromBinary;
use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use crate::imdb::tsv_import::tsv_import;
use crate::utils::io::Progress;
use crate::utils::result::Res;
use crate::utils::search::SearchString;

use flate2::bufread::GzDecoder;
use humantime::format_duration;
use log::{debug, log_enabled};
use reqwest::blocking::{Client, Response};
use reqwest::Url;

/// Struct providing the movies and series databases and the related services
pub struct Service {
  service_db: ServiceDbFromBinary,
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
  pub fn new(cache_dir: &Path, force_db_update: bool, progress_fn: impl Fn(Option<u64>, u64)) -> Res<Self> {
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
    let service = Self { service_db: ServiceDbFromBinary::new(movies_data, series_data) };
    debug!("Parsed IMDB database in {}", format_duration(Instant::now().duration_since(start)));

    if log_enabled!(log::Level::Debug) {
      let (total_movies, total_series) = service.service_db.n_entries();
      let total_entries = total_movies + total_series;
      debug!(
        "IMDB database contains {total_movies} movies and {total_series} series ({total_entries} entries)"
      );
    }

    Ok(service)
  }

  /// Returns the file at the given path if it exists, or an Ok Result if it is not found.
  /// Only returns an error if a problem occurs while opening an existing file.
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
  fn create_downloader(resp: Response, progress_fn: impl Fn(Option<u64>, u64)) -> impl BufRead {
    let progress = Progress::new(resp, progress_fn);
    let reader = BufReader::new(progress);
    let decoder = GzDecoder::new(reader);
    BufReader::new(decoder)
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
    progress_fn: impl Fn(Option<u64>, u64),
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

      let basics_downloader = Self::create_downloader(basics_resp, &progress_fn);
      let ratings_downloader = Self::create_downloader(ratings_resp, &progress_fn);

      let movies_db_file = File::create(movies_db_filename)?;
      let movies_db_writer = BufWriter::new(movies_db_file);

      let series_db_file = File::create(series_db_filename)?;
      let series_db_writer = BufWriter::new(series_db_file);

      tsv_import(ratings_downloader, basics_downloader, movies_db_writer, series_db_writer)?;
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
    self.service_db.by_id(id, query)
  }

  /// Query titles by title
  /// # Arguments
  /// * `title` - Title to be queried
  /// * `query` - Specifies if movies or series are queried
  pub fn by_title(&self, title: &SearchString, query: Query) -> Vec<&Title> {
    self.service_db.by_title(title, query)
  }

  /// Query titles by title and year
  /// # Arguments
  /// * `title` - Title to be queried
  /// * `year` - Release year of the title
  /// * `query` - Specifies if movies or series are queried
  pub fn by_title_and_year(&self, title: &SearchString, year: u16, query: Query) -> Vec<&Title> {
    self.service_db.by_title_and_year(title, year, query)
  }

  /// Query titles by keywords
  /// # Arguments
  /// * `keywords` - List of keywords to search in titles
  /// * `query` - Specifies if movies or series are queried
  pub fn by_keywords<'a, 'k>(&'a self, keywords: &'k [SearchString], query: Query) -> Vec<&'a Title> {
    self.service_db.by_keywords(keywords, query)
  }

  /// Query titles by keywords and year
  /// # Arguments
  /// * `keywords` - List of keywords to search in titles
  /// * `year` - Release year of the title
  /// * `query` - Specifies if movies or series are queried
  pub fn by_keywords_and_year<'a, 'k>(
    &'a self,
    keywords: &'k [SearchString],
    year: u16,
    query: Query,
  ) -> Vec<&'a Title> {
    self.service_db.by_keywords_and_year(keywords, year, query)
  }
}
