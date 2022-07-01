use super::db::Db;
use crate::imdb::db::Query;
use crate::imdb::title::Title;
use crate::imdb::title_id::TitleId;
use log::debug;
use parking_lot::{const_mutex, Mutex};
use rayon::prelude::*;

pub struct ServiceDbFromBinary {
  dbs: Vec<Db>,
}

impl ServiceDbFromBinary {
  /// Load titles from the given binary data.
  ///
  /// Loads titles from the provided binary content buffers (`movies_data` and
  /// `series_data`) into one of the thread-handled databases.
  ///
  /// # Arguments
  ///
  /// * `movies_data` - Binary movies data.
  /// * `series_data` - Binary series data.
  pub fn new(mut movies_data: &'static [u8], mut series_data: &'static [u8]) -> Self {
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

  /// Loads titles from the provided binary content buffers into the thread-handled
  /// databases.
  ///
  /// # Arguments
  ///
  /// * `cursor` - Cursor at the binary to read the titles from.
  /// * `titles` - Vector to store the titles temporarily before writing to the database.
  /// * `db` - Database to store movies or series.
  fn titles_from_binary<const IS_MOVIE: bool>(
    cursor: &Mutex<&mut &'static [u8]>,
    titles: &mut Vec<Title<'static>>,
    db: &mut Db,
  ) {
    loop {
      let mut cursor_guard = cursor.lock();

      if (*cursor_guard).is_empty() {
        break;
      }

      for _ in 0..100 {
        if (*cursor_guard).is_empty() {
          break;
        }

        let title = match Title::from_binary(&mut cursor_guard) {
          Ok(title) => title,
          Err(e) => panic!("Error parsing title: {}", e),
        };

        titles.push(title);
      }

      drop(cursor_guard);

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

  /// Calculate the total number of movies and series entries.
  ///
  /// # Return
  ///
  /// Returns a tuple containing two numbers, the first one is the number of movies and
  /// the second on the number of series contained in the database.
  pub fn n_entries(&self) -> (usize, usize) {
    let mut total_movies = 0;
    let mut total_series = 0;

    for (i, db) in self.dbs.iter().enumerate() {
      let movies = db.n_movies();
      let series = db.n_series();
      let entries = db.n_entries();

      total_movies += movies;
      total_series += series;

      debug!("IMDB database (thread {i}) contains {movies} movies and {series} series ({entries} entries)");
    }

    (total_movies, total_series)
  }

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

  pub fn by_title(&self, title: &str, query: Query) -> Vec<&Title> {
    self
      .dbs
      .par_iter()
      .map(|db| db.by_title(title, query).collect::<Vec<_>>())
      .flatten()
      .collect()
  }
}

#[cfg(test)]
mod tests {
  use crate::imdb::db::Query;
  use crate::imdb::db::ServiceDb;
  use crate::imdb::db_new::ServiceDbFromBinary;
  use crate::imdb::title_id::TitleId;
  use indoc::indoc;
  use std::io::BufRead;

  fn make_basics_reader() -> impl BufRead {
    indoc! {"
      tconst\ttitleType\tprimaryTitle\toriginalTitle\tisAdult\tstartYear\tendYear\truntimeMinutes\tgenres
      tt0000001\tshort\tCarmencita\tCarmencita\t0\t1894\t\\N\t1\tDocumentary,Short
      tt0000002\tshort\tLe clown et ses chiens\tLe clown et ses chiens\t0\t1892\t\\N\t5\tAnimation,Short
      tt0000003\tshort\tPauvre Pierrot\tPauvre Pierrot\t0\t1892\t\\N\t4\tAnimation,Comedy,Romance
      tt0000004\tshort\tUn bon bock\tUn bon bock\t0\t1892\t\\N\t12\tAnimation,Short
      tt0000005\tshort\tBlacksmith Scene\tBlacksmith Scene\t0\t1893\t\\N\t1\tComedy,Short
      tt0000006\tshort\tChinese Opium Den\tChinese Opium Den\t0\t1894\t\\N\t1\tShort
      tt0000007\tshort\tCorbett and Courtney Before the Kinetograph\tCorbett and Courtney Before the Kinetograph\t0\t1894\t\\N\t1\tShort,Sport
      tt0000008\tshort\tEdison Kinetoscopic Record of a Sneeze\tEdison Kinetoscopic Record of a Sneeze\t0\t1894\t\\N\t1\tDocumentary,Short
      tt0000009\tshort\tMiss Jerry\tMiss Jerry\t0\t1894\t\\N\t40\tRomance,Short
      tt0000010\tshort\tLeaving the Factory\tLa sortie de l'usine Lumière à Lyon\t0\t1895\t\\N\t1\tDocumentary,Short
    "}.as_bytes()
  }

  fn make_ratings_reader() -> impl BufRead {
    indoc! {"
      tconst\taverageRating\tnumVotes
      tt0000001\t5.7\t1845
      tt0000002\t6.0\t236
      tt0000003\t6.5\t1603
      tt0000004\t6.0\t153
      tt0000005\t6.2\t2424
      tt0000006\t5.2\t158
      tt0000007\t5.4\t758
      tt0000008\t5.5\t1988
      tt0000009\t5.9\t191
      tt0000010\t6.9\t6636
    "}
    .as_bytes()
  }

  fn make_service_db_from_binary() -> ServiceDbFromBinary {
    let basics_reader = make_basics_reader();
    let ratings_reader = make_ratings_reader();

    let mut movies_storage = Vec::new();
    let mut series_storage = Vec::new();
    ServiceDb::import(ratings_reader, basics_reader, &mut movies_storage, &mut series_storage).unwrap();

    let movies_storage = Box::leak(movies_storage.into_boxed_slice());
    let series_storage = Box::leak(series_storage.into_boxed_slice());
    ServiceDbFromBinary::new(movies_storage, series_storage)
  }

  #[test]
  fn test_n_entries() {
    let service_db = make_service_db_from_binary();
    assert_eq!(service_db.n_entries(), (10, 0));
  }

  #[test]
  fn test_by_id() {
    let service_db = make_service_db_from_binary();
    let title = service_db
      .by_id(&TitleId::try_from("tt0000007").unwrap(), Query::Movies)
      .unwrap();
    assert_eq!(title.title_id(), &TitleId::try_from("tt0000007").unwrap());
    assert_eq!(title.primary_title(), "Corbett and Courtney Before the Kinetograph");
  }

  #[test]
  fn test_by_title() {
    let service_db = make_service_db_from_binary();
    let titles = service_db
      .by_title("Corbett and Courtney Before the Kinetograph", Query::Movies);
    assert_eq!(titles.len(), 1);
    let title = titles[0];
    assert_eq!(title.title_id(), &TitleId::try_from("tt0000007").unwrap());
    assert_eq!(title.primary_title(), "Corbett and Courtney Before the Kinetograph");
  }
}
