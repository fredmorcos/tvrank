#![warn(clippy::all)]

use std::path::Path;
use std::time::{Duration, Instant};

use crate::imdb::db::Query;
use crate::imdb::db_binary::ServiceDbFromBinary;
use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use crate::imdb::tsv_import::tsv_import;
use crate::utils::io::file as io_file;
use crate::utils::io::net as io_net;
use crate::utils::result::Res;
use crate::utils::search::SearchString;

use humantime::format_duration;
use log::{debug, log_enabled};
use reqwest::Url;

/// Struct providing the movies and series databases and the related services
pub struct Service {
  service_db: ServiceDbFromBinary,
}

const IMDB_URL: &str = "https://datasets.imdbws.com/";
const BASICS_FILENAME: &str = "title.basics.tsv.gz";
const RATINGS_FILENAME: &str = "title.ratings.tsv.gz";

const MOVIES_DB_FILENAME: &str = "imdb-movies.tvrankdb";
const SERIES_DB_FILENAME: &str = "imdb-series.tvrankdb";

impl Service {
  /// Returns a Service struct holding movies/series databases
  ///
  /// # Arguments
  ///
  /// * `cache_dir` - Directory path of the database files.
  /// * `force_db_update` - True if the databases should be updated regardless of their age.
  /// * `progress_fn` - Function that keeps track of the download progress.
  pub fn new(cache_dir: &Path, force_db_update: bool, progress_fn: impl Fn(Option<u64>, u64)) -> Res<Self> {
    let one_month = Duration::from_secs(60 * 60 * 24 * 30);

    let movies_db_filename = cache_dir.join(MOVIES_DB_FILENAME);
    let series_db_filename = cache_dir.join(SERIES_DB_FILENAME);
    Self::ensure_db_files(&movies_db_filename, &series_db_filename, one_month, force_db_update, progress_fn)?;

    let start = Instant::now();
    let movies_data = io_file::read_static(&movies_db_filename)?;
    let series_data = io_file::read_static(&series_db_filename)?;
    debug!("Read IMDB database in {}", format_duration(Instant::now().duration_since(start)));

    let start = Instant::now();
    let service = Self { service_db: ServiceDbFromBinary::new(movies_data, series_data) };
    debug!("Parsed IMDB database in {}", format_duration(Instant::now().duration_since(start)));

    if log_enabled!(log::Level::Debug) {
      let (total_movies, total_series) = service.service_db.n_entries();
      let total_entries = total_movies + total_series;
      debug!("IMDB: {total_movies} movies and {total_series} series ({total_entries} entries)");
    }

    Ok(service)
  }

  /// Ensures that the movies and series databases exist and are up-to-date.
  ///
  /// The databases are created if they don't exist, and updated if they are outdated or
  /// if the force_db_update parameter is set to true.
  ///
  /// # Arguments
  ///
  /// * `movies_db_filename` - Path to the movies database.
  /// * `series_db_filename` - Path to the series database.
  /// * `force_db_update` - True if the databases should be updated regardless of their age.
  /// * `progress_fn` - Function that keeps track of the download progress.
  fn ensure_db_files(
    movies_db_filename: &Path,
    series_db_filename: &Path,
    max_age: Duration,
    force_db_update: bool,
    progress_fn: impl Fn(Option<u64>, u64),
  ) -> Res {
    let needs_update = {
      force_db_update
        || io_file::older_than(&io_file::open_existing(movies_db_filename)?, max_age)
        || io_file::older_than(&io_file::open_existing(series_db_filename)?, max_age)
    };

    if needs_update {
      if force_db_update {
        debug!("Force-update is enabled, IMDB database is going to be re-fetched and built");
      } else {
        debug!("IMDB database does not exist or is more than a month old, going to fetch and build");
      }

      let movies_db_writer = io_file::create_buffered(movies_db_filename)?;
      let series_db_writer = io_file::create_buffered(series_db_filename)?;

      let imdb_url = Url::parse(IMDB_URL)?;
      let basics_response = io_net::get_response(imdb_url.join(BASICS_FILENAME)?)?;
      let ratings_response = io_net::get_response(imdb_url.join(RATINGS_FILENAME)?)?;

      let content_length = match (basics_response.content_length(), ratings_response.content_length()) {
        (None, _) | (_, None) => None,
        (Some(basics_content_length), Some(ratings_content_length)) => {
          Some(basics_content_length + ratings_content_length)
        }
      };

      progress_fn(content_length, 0);

      let basics_fetcher = io_net::make_fetcher(basics_response, |bytes| progress_fn(None, bytes));
      let ratings_fetcher = io_net::make_fetcher(ratings_response, |bytes| progress_fn(None, bytes));

      tsv_import(ratings_fetcher, basics_fetcher, movies_db_writer, series_db_writer)?;
    } else {
      debug!("IMDB database exists and is less than a month old");
    }

    Ok(())
  }

  /// Query titles by ID.
  ///
  /// # Arguments
  ///
  /// * `id` - ID of the title to be queried.
  /// * `query` - Specifies if movies or series are queried.
  pub fn by_id(&self, id: &TitleId, query: Query) -> Option<&Title> {
    self.service_db.by_id(id, query)
  }

  /// Query titles by title.
  ///
  /// # Arguments
  ///
  /// * `title` - Title to be queried.
  /// * `query` - Specifies if movies or series are queried.
  pub fn by_title(&self, title: &SearchString, query: Query) -> Vec<&Title> {
    self.service_db.by_title(title, query)
  }

  /// Query titles by title and year.
  ///
  /// # Arguments
  ///
  /// * `title` - Title to be queried.
  /// * `year` - Release year of the title.
  /// * `query` - Specifies if movies or series are queried.
  pub fn by_title_and_year(&self, title: &SearchString, year: u16, query: Query) -> Vec<&Title> {
    self.service_db.by_title_and_year(title, year, query)
  }

  /// Query titles by keywords.
  ///
  /// # Arguments
  ///
  /// * `keywords` - List of keywords to search in titles.
  /// * `query` - Specifies if movies or series are queried.
  pub fn by_keywords<'a, 'k>(&'a self, keywords: &'k [SearchString], query: Query) -> Vec<&'a Title> {
    self.service_db.by_keywords(keywords, query)
  }

  /// Query titles by keywords and year.
  ///
  /// # Arguments
  ///
  /// * `keywords` - List of keywords to search in titles.
  /// * `year` - Release year of the title.
  /// * `query` - Specifies if movies or series are queried.
  pub fn by_keywords_and_year<'a, 'k>(
    &'a self,
    keywords: &'k [SearchString],
    year: u16,
    query: Query,
  ) -> Vec<&'a Title> {
    self.service_db.by_keywords_and_year(keywords, year, query)
  }
}
