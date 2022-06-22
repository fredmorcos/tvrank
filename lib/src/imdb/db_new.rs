use super::db::Db;
use parking_lot::{const_mutex, Mutex};

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

  pub fn n_entries(&self) -> (usize, usize) {
    (10, 0)
  }
}

#[cfg(test)]
mod tests {
  use crate::imdb::db::ServiceDb;
  use crate::imdb::db_new::ServiceDbFromBinary;
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

  #[test]
  fn service_db_from_binary() {
    let basics_reader = make_basics_reader();
    let ratings_reader = make_ratings_reader();

    let mut movies_storage = Vec::new();
    let mut series_storage = Vec::new();

    ServiceDb::import(ratings_reader, basics_reader, &mut movies_storage, &mut series_storage).unwrap();

    let service_db = ServiceDbFromBinary::new(movies_storage.as_slice(), series_storage.as_slice());
    assert_eq!(service_db.n_entries(), (10, 0));
  }
}
