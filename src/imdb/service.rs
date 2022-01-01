#![warn(clippy::all)]

use super::basics::{Basics, QueryType};
use super::error::Err;
use super::parsing::LINES_PER_THREAD;
use super::ratings::Ratings;
use super::storage::Storage;
use super::title::{Title, TitleId};
use crate::Res;
use fnv::FnvHashSet;
use humantime::format_duration;
use log::{debug, error, info};
use parking_lot::const_mutex;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Instant;

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
    let mut total_titles = 0;

    for (i, db) in basics_dbs.iter().enumerate() {
      let n_movies = db.n_movies();
      let n_series = db.n_series();
      let n_titles = db.n_titles();

      total_movies += n_movies;
      total_series += n_series;
      total_titles += n_titles;

      debug!("DB {} has {} movies and {} series ({} titles)", i, n_movies, n_series, n_titles,);
    }

    debug!("DB has a total of {} movies and {} series ({} titles)", total_movies, total_series, total_titles,);

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
            lines.extend(basics_source.lock().deref_mut().take(LINES_PER_THREAD));

            if lines.is_empty() {
              break;
            }

            for &line in &lines {
              if !line.is_empty() {
                if let Err(e) = basics_db.add(line) {
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

  pub fn by_id(&self, id: &TitleId, query_type: QueryType) -> Res<Vec<Title>> {
    let res = Arc::new(const_mutex(vec![]));

    rayon::scope(|scope| {
      for db in self.basics_dbs.as_slice() {
        scope.spawn(|_| {
          let titles = db
            .by_id(id, query_type)
            .map(|basics| Title::new(basics, self.ratings_db.get(&basics.title_id)));
          let mut res = res.lock();
          res.extend(titles);
        })
      }
    });

    match Arc::try_unwrap(res) {
      Ok(res) => Ok(res.into_inner()),
      Err(_) => {
        error!("Failed to unwrap an Arc containing the search results, this should not happen");
        Err::basics_db_query()
      }
    }
  }

  pub fn by_title(&self, title: &str, query_type: QueryType) -> Res<Vec<Title>> {
    let res = Arc::new(const_mutex(vec![]));

    rayon::scope(|scope| {
      for db in self.basics_dbs.as_slice() {
        scope.spawn(|_| {
          let titles = db
            .by_title(title, query_type)
            .map(|basics| Title::new(basics, self.ratings_db.get(&basics.title_id)));
          let mut res = res.lock();
          res.extend(titles);
        })
      }
    });

    match Arc::try_unwrap(res) {
      Ok(res) => Ok(res.into_inner()),
      Err(_) => {
        error!("Failed to unwrap an Arc containing the search results, this should not happen");
        Err::basics_db_query()
      }
    }
  }

  pub fn by_title_and_year(&self, title: &str, year: u16, query_type: QueryType) -> Res<Vec<Title>> {
    let res = Arc::new(const_mutex(vec![]));

    rayon::scope(|scope| {
      for db in self.basics_dbs.as_slice() {
        scope.spawn(|_| {
          let titles = db
            .by_title_and_year(title, year, query_type)
            .map(|basics| Title::new(basics, self.ratings_db.get(&basics.title_id)));
          let mut res = res.lock();
          res.extend(titles);
        })
      }
    });

    match Arc::try_unwrap(res) {
      Ok(res) => Ok(res.into_inner()),
      Err(_) => {
        error!("Failed to unwrap an Arc containing the search results, this should not happen");
        Err::basics_db_query()
      }
    }
  }

  pub fn by_keywords<'a>(&'a self, keywords: &'a [&str], query_type: QueryType) -> Res<Vec<Title<'a, 'a>>> {
    let res = Arc::new(const_mutex(vec![]));

    rayon::scope(|scope| {
      for db in self.basics_dbs.as_slice() {
        scope.spawn(|_| {
          let title_basics: FnvHashSet<_> = db.by_keywords(keywords, query_type).collect();
          let titles = title_basics
            .iter()
            .map(|basics| Title::new(basics, self.ratings_db.get(&basics.title_id)));
          let mut res = res.lock();
          res.extend(titles);
        })
      }
    });

    match Arc::try_unwrap(res) {
      Ok(res) => Ok(res.into_inner()),
      Err(_) => {
        error!("Failed to unwrap an Arc containing the search results, this should not happen");
        Err::basics_db_query()
      }
    }
  }
}
