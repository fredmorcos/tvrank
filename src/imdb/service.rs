#![warn(clippy::all)]

use super::basics::Basics;
use super::error::Err;
use super::ratings::Ratings;
use super::storage::Storage;
use super::title::{Title, TitleId};
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

#[derive(Clone, Copy)]
pub enum QueryType {
  Movies,
  Series,
}

type TitleResults<'a, 'b> = Mutex<Vec<Title<'a, 'b>>>;

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

  fn query_by_title(
    &self,
    name: &str,
    year: Option<u16>,
    query_fn: for<'a, 'b> fn(&str, Option<u16>, &'a Basics, &'b Ratings, &TitleResults<'a, 'b>),
  ) -> Res<Vec<Title>> {
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

        scope.spawn(move |_| query_fn(name, year, db, &self.ratings_db, &res));
      }
    });

    let res = if let Ok(res) = Arc::try_unwrap(res) {
      res.into_inner()
    } else {
      return Err::basics_db_query();
    };

    Ok(res)
  }

  fn movies_by_title(&self, name: &str, year: Option<u16>) -> Res<Vec<Title>> {
    fn query_fn<'a, 'b>(
      name: &str,
      year: Option<u16>,
      basics: &'a Basics,
      ratings: &'b Ratings,
      res: &TitleResults<'a, 'b>,
    ) {
      if let Some(year) = year {
        let local_res = basics
          .movies_by_title_year(name, year)
          .map(|b| Title::new(b, ratings.get(&b.title_id)));
        let mut res = res.lock();
        res.extend(local_res);
      } else {
        let local_res = basics.movies_by_title(name).map(|b| Title::new(b, ratings.get(&b.title_id)));
        let mut res = res.lock();
        res.extend(local_res);
      }
    }

    self.query_by_title(name, year, query_fn)
  }

  fn series_by_title(&self, name: &str, year: Option<u16>) -> Res<Vec<Title>> {
    fn query_fn<'a, 'b>(
      name: &str,
      year: Option<u16>,
      basics: &'a Basics,
      ratings: &'b Ratings,
      res: &TitleResults<'a, 'b>,
    ) {
      if let Some(year) = year {
        let local_res = basics
          .series_by_title_year(name, year)
          .map(|b| Title::new(b, ratings.get(&b.title_id)));
        let mut res = res.lock();
        res.extend(local_res);
      } else {
        let local_res = basics.series_by_title(name).map(|b| Title::new(b, ratings.get(&b.title_id)));
        let mut res = res.lock();
        res.extend(local_res);
      }
    }

    self.query_by_title(name, year, query_fn)
  }

  pub fn by_title(&self, query_type: QueryType, name: &str, year: Option<u16>) -> Res<Vec<Title>> {
    match query_type {
      QueryType::Movies => self.movies_by_title(name, year),
      QueryType::Series => self.series_by_title(name, year),
    }
  }

  fn query_by_titleid(
    &self,
    title_id: &TitleId,
    query_fn: for<'a, 'b> fn(&TitleId, &'a Basics, &'b Ratings, &TitleResults<'a, 'b>),
  ) -> Res<Vec<Title>> {
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

        scope.spawn(move |_| query_fn(title_id, db, &self.ratings_db, &res));
      }
    });

    let res = if let Ok(res) = Arc::try_unwrap(res) {
      res.into_inner()
    } else {
      return Err::basics_db_query();
    };

    Ok(res)
  }

  fn movie_by_titleid(&self, title_id: &TitleId) -> Res<Vec<Title>> {
    fn query_fn<'a, 'b>(
      title_id: &TitleId,
      basics: &'a Basics,
      ratings: &'b Ratings,
      res: &TitleResults<'a, 'b>,
    ) {
      if let Some(b) = basics.movie_by_titleid(title_id) {
        let local_res = Title::new(b, ratings.get(&b.title_id));
        let mut res = res.lock();
        res.push(local_res);
      }
    }

    self.query_by_titleid(title_id, query_fn)
  }

  fn series_by_titleid(&self, title_id: &TitleId) -> Res<Vec<Title>> {
    fn query_fn<'a, 'b>(
      title_id: &TitleId,
      basics: &'a Basics,
      ratings: &'b Ratings,
      res: &TitleResults<'a, 'b>,
    ) {
      if let Some(b) = basics.series_by_titleid(title_id) {
        let local_res = Title::new(b, ratings.get(&b.title_id));
        let mut res = res.lock();
        res.push(local_res);
      }
    }

    self.query_by_titleid(title_id, query_fn)
  }

  pub fn by_titleid(&self, query_type: QueryType, title_id: &TitleId) -> Res<Vec<Title>> {
    match query_type {
      QueryType::Movies => self.movie_by_titleid(title_id),
      QueryType::Series => self.series_by_titleid(title_id),
    }
  }

  fn query_by_keywords<'a>(
    &'a self,
    keywords: &'a [&str],
    query_fn: fn(&'a [&str], &'a Basics, &'a Ratings, &TitleResults<'a, 'a>),
  ) -> Res<Vec<Title>> {
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

        scope.spawn(move |_| query_fn(keywords, db, &self.ratings_db, &res));
      }
    });

    let res = if let Ok(res) = Arc::try_unwrap(res) {
      res.into_inner()
    } else {
      return Err::basics_db_query();
    };

    Ok(res)
  }

  fn movies_by_keywords<'a>(&'a self, keywords: &'a [&str]) -> Res<Vec<Title<'a, 'a>>> {
    fn query_fn<'a>(
      keywords: &'a [&str],
      basics: &'a Basics,
      ratings: &'a Ratings,
      res: &TitleResults<'a, 'a>,
    ) {
      let local_res = basics
        .movies_by_keyword(keywords)
        .map(|b| Title::new(b, ratings.get(&b.title_id)));
      let mut res = res.lock();
      res.extend(local_res)
    }

    self.query_by_keywords(keywords, query_fn)
  }

  fn series_by_keywords<'a>(&'a self, keywords: &'a [&str]) -> Res<Vec<Title<'a, 'a>>> {
    fn query_fn<'a>(
      keywords: &'a [&str],
      basics: &'a Basics,
      ratings: &'a Ratings,
      res: &TitleResults<'a, 'a>,
    ) {
      let local_res = basics
        .series_by_keyword(keywords)
        .map(|b| Title::new(b, ratings.get(&b.title_id)));
      let mut res = res.lock();
      res.extend(local_res)
    }

    self.query_by_keywords(keywords, query_fn)
  }

  pub fn by_keywords<'a>(&'a self, query_type: QueryType, keywords: &'a [&str]) -> Res<Vec<Title<'a, 'a>>> {
    match query_type {
      QueryType::Movies => self.movies_by_keywords(keywords),
      QueryType::Series => self.series_by_keywords(keywords),
    }
  }
}
