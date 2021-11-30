#![warn(clippy::all)]

use super::basics::Basics;
use super::title::Title;
use crate::imdb::{ratings::Ratings, storage::Storage};
use crate::Res;
use crossbeam::thread;
use deepsize::DeepSizeOf;
use indicatif::HumanBytes;
use log::{debug, error, info};
use parking_lot::const_mutex;
use std::convert::TryInto;
use std::{ops::DerefMut, sync::Arc};

#[derive(DeepSizeOf)]
pub struct Service {
  basics_dbs: Vec<Basics>,
  ratings_db: Ratings,
}

impl Service {
  pub fn new(storage: &Storage) -> Res<Self> {
    let ncpus = num_cpus::get() / 2;
    debug!("Going to use {} threads", ncpus);

    info!("Parsing IMDB Basics DB...");
    let basics_dbs = Self::parse_basics(ncpus, storage)?;
    info!("Done parsing IMDB Basics DB");

    info!("Parsing IMDB Ratings DB...");
    let ratings_db = Ratings::new_from_buf(storage.ratings_db_buf)?;
    info!("Done parsing IMDB Ratings DB");

    let mut total_movies = 0;
    let mut total_series = 0;
    let mut total_size = 0;

    for (i, db) in basics_dbs.iter().enumerate() {
      let n_movies = db.n_movies();
      let n_series = db.n_series();
      total_movies += n_movies;
      total_series += n_series;

      let size = db.deep_size_of();
      total_size += size;

      debug!(
        "DB {} has {} movies and {} series (Mem: {})",
        i,
        n_movies,
        n_series,
        HumanBytes(size.try_into()?)
      );
    }

    debug!(
      "DB has a total of {} movies and {} series (Mem: {})",
      total_movies,
      total_series,
      HumanBytes(total_size.try_into()?)
    );

    Ok(Self { basics_dbs, ratings_db })
  }

  fn parse_basics(n: usize, storage: &Storage) -> Res<Vec<Basics>> {
    let mut basics_dbs = Vec::with_capacity(n);

    let basics_source = storage.basics_db_buf.split(|&b| b == b'\n').skip(1);
    let basics_source = Arc::new(const_mutex(basics_source));

    let _ = thread::scope(|s| {
      let mut handles = Vec::with_capacity(n);

      for i in 0..n {
        let basics_source = basics_source.clone();

        let handle = s.builder().name(format!("IMDB Parsing Thread #{}", i)).spawn(move |_| {
          let mut basics_db = Basics::default();
          let mut lines = vec![];

          loop {
            lines.extend(basics_source.lock().deref_mut().take(200_000));

            if lines.is_empty() {
              break;
            }

            for &line in &lines {
              if !line.is_empty() {
                if let Err(e) = basics_db.add_basics_from_line(line) {
                  panic!("On DB Thread {}: Cannot parse DB: {}", i, e)
                }
              }
            }

            lines.clear();
          }

          basics_db
        });

        match handle {
          Ok(handle) => handles.push(handle),
          Err(e) => error!("Could not spawn thread {} for parsing DB: {}", i, e),
        }
      }

      for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
          Ok(basics_db) => basics_dbs.push(basics_db),
          Err(e) => error!("Could not join thread {}: {:?}", i, e),
        }
      }
    });

    Ok(basics_dbs)
  }

  fn query(
    &self,
    query_fn: impl for<'a, 'b> Fn(&'a Basics, &'b Ratings) -> Vec<Title<'a, 'b>> + Send + Sync,
  ) -> Res<Vec<Vec<Title>>> {
    let mut res = Vec::with_capacity(self.basics_dbs.len());

    let _ = thread::scope(|s| {
      let mut handles = Vec::with_capacity(self.basics_dbs.len());
      let mut dbs: &[Basics] = self.basics_dbs.as_slice();

      for i in 0..self.basics_dbs.len() {
        let (db, rem) = match dbs.split_first() {
          Some(res) => res,
          None => break,
        };

        dbs = rem;

        let thread_name = format!("IMDB Query Thread #{}", i);
        let handle = s.builder().name(thread_name).spawn(|_| query_fn(db, &self.ratings_db));

        match handle {
          Ok(handle) => handles.push(handle),
          Err(e) => error!("Could not spawn thread {} for movie querying: {}", i, e),
        }
      }

      for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
          Ok(titles) => res.push(titles),
          Err(e) => error!("Could not join thread {}: {:?}", i, e),
        }
      }
    });

    Ok(res)
  }

  pub fn movies_by_title(&self, name: &str, year: Option<u16>) -> Res<Vec<Vec<Title>>> {
    self.query(|basics_db, ratings_db| {
      if let Some(year) = year {
        if let Some(titles) = basics_db.movies_by_title_with_year(name, year) {
          titles.map(|b| Title::new(b, ratings_db.get(&b.title_id))).collect()
        } else {
          vec![]
        }
      } else if let Some(titles) = basics_db.movies_by_title(name) {
        titles.map(|b| Title::new(b, ratings_db.get(&b.title_id))).collect()
      } else {
        vec![]
      }
    })
  }

  pub fn series_by_title(&self, name: &str, year: Option<u16>) -> Res<Vec<Vec<Title>>> {
    self.query(|basics_db, ratings_db| {
      if let Some(year) = year {
        if let Some(titles) = basics_db.series_by_title_with_year(name, year) {
          titles.map(|b| Title::new(b, ratings_db.get(&b.title_id))).collect()
        } else {
          vec![]
        }
      } else if let Some(titles) = basics_db.series_by_title(name) {
        titles.map(|b| Title::new(b, ratings_db.get(&b.title_id))).collect()
      } else {
        vec![]
      }
    })
  }
}
