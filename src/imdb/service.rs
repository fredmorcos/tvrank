#![warn(clippy::all)]

use super::basics::Basics;
use super::title::Title;
use crate::imdb::{error::Err, ratings::Ratings, storage::Storage};
use crate::Res;
use deepsize::DeepSizeOf;
use humantime::format_duration;
use indicatif::HumanBytes;
use log::{debug, info};
use parking_lot::{const_mutex, Mutex};
use std::convert::TryInto;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Instant;

#[derive(DeepSizeOf)]
pub struct Service {
  basics_dbs: Vec<Basics>,
  ratings_db: Ratings,
}

impl Service {
  pub fn new(ncpus: usize, storage: &Storage) -> Res<Self> {
    debug!("Going to use {} threads", ncpus);

    info!("Parsing IMDB Basics DB...");
    let start_time = Instant::now();
    let basics_dbs = Self::parse_basics(ncpus, storage)?;
    info!("Done parsing IMDB Basics DB in {}", format_duration(Instant::now().duration_since(start_time)));

    info!("Parsing IMDB Ratings DB...");
    let start_time = Instant::now();
    let ratings_db = Ratings::new_from_buf(storage.ratings)?;
    info!("Done parsing IMDB Ratings DB in {}", format_duration(Instant::now().duration_since(start_time)));

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
    let basics_dbs = Arc::new(const_mutex(Vec::with_capacity(n)));

    let basics_source = storage.basics.split(|&b| b == b'\n').skip(1);
    let basics_source = Arc::new(const_mutex(basics_source));

    rayon::scope(|scope| {
      for thread_idx in 0..n {
        let basics_source = basics_source.clone();
        let basics_dbs = Arc::clone(&basics_dbs);

        scope.spawn(move |_| {
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
                  panic!("On DB Thread {}: Cannot parse DB: {}", thread_idx, e)
                }
              }
            }

            lines.clear();
          }

          let mut basics_dbs = basics_dbs.lock();
          basics_dbs.push(basics_db);
        });
      }
    });

    let basics_dbs = if let Ok(basics_dbs) = Arc::try_unwrap(basics_dbs) {
      basics_dbs.into_inner()
    } else {
      return Err::basics_db_build();
    };

    Ok(basics_dbs)
  }

  fn query(
    &self,
    query_fn: impl for<'a, 'b> Fn(&'a Basics, &'b Ratings, &Mutex<Vec<Vec<Title<'a, 'b>>>>) + Send + Sync + Copy,
  ) -> Res<Vec<Vec<Title>>> {
    let res = Arc::new(const_mutex(Vec::with_capacity(self.basics_dbs.len())));

    rayon::scope(|scope| {
      let mut dbs: &[Basics] = self.basics_dbs.as_slice();

      for _ in 0..self.basics_dbs.len() {
        let res = Arc::clone(&res);

        let (db, rem) = match dbs.split_first() {
          Some(res) => res,
          None => break,
        };

        dbs = rem;

        scope.spawn(move |_| query_fn(db, &self.ratings_db, &res));
      }
    });

    let res = if let Ok(res) = Arc::try_unwrap(res) {
      res.into_inner()
    } else {
      return Err::basics_db_query();
    };

    Ok(res)
  }

  pub fn movies_by_title(&self, name: &str, year: Option<u16>) -> Res<Vec<Vec<Title>>> {
    self.query(|basics_db, ratings_db, res| {
      if let Some(year) = year {
        if let Some(titles) = basics_db.movies_by_title_with_year(name, year) {
          let local_res = titles
            .map(|b| Title::new(b, ratings_db.get(&b.title_id)))
            .collect::<Vec<Title>>();
          let mut res = res.lock();
          res.push(local_res);
        }
      } else if let Some(titles) = basics_db.movies_by_title(name) {
        let local_res = titles
          .map(|b| Title::new(b, ratings_db.get(&b.title_id)))
          .collect::<Vec<Title>>();
        let mut res = res.lock();
        res.push(local_res);
      }
    })
  }

  pub fn series_by_title(&self, name: &str, year: Option<u16>) -> Res<Vec<Vec<Title>>> {
    self.query(|basics_db, ratings_db, res| {
      if let Some(year) = year {
        if let Some(titles) = basics_db.series_by_title_with_year(name, year) {
          let local_res = titles
            .map(|b| Title::new(b, ratings_db.get(&b.title_id)))
            .collect::<Vec<Title>>();
          let mut res = res.lock();
          res.push(local_res);
        }
      } else if let Some(titles) = basics_db.series_by_title(name) {
        let local_res = titles
          .map(|b| Title::new(b, ratings_db.get(&b.title_id)))
          .collect::<Vec<Title>>();
        let mut res = res.lock();
        res.push(local_res);
      }
    })
  }
}
