#![warn(clippy::all)]

use super::{basics::Basics, title::Title};
use crate::{imdb::storage::Storage, Res};
use log::info;
use std::path::Path;

pub struct Service {
  basics_db: Basics,
}

impl Service {
  pub fn new(app_cache_dir: &Path) -> Res<Self> {
    info!("Loading IMDB Databases...");
    let storage = Storage::load_db_files(app_cache_dir)?;

    info!("Parsing IMDB Basics DB...");
    let mut basics_db = Basics::default();
    for line in storage.basics_db_buf.split(|&b| b == b'\n').skip(1) {
      if !line.is_empty() {
        basics_db.add_from_line(line)?;
      }
    }
    info!("Done loading IMDB Basics DB");

    Ok(Service { basics_db })
  }

  pub fn movie(&self, name: &str, year: Option<u16>) -> Option<Vec<&Title>> {
    if let Some(year) = year {
      self.basics_db.movie_with_year(name, year)
    } else {
      self.basics_db.movie(name)
    }
  }

  pub fn series(&self, name: &str, year: Option<u16>) -> Option<Vec<&Title>> {
    if let Some(year) = year {
      self.basics_db.series_with_year(name, year)
    } else {
      self.basics_db.series(name)
    }
  }
}
