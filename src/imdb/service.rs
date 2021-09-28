#![warn(clippy::all)]

use super::{basics::Basics, title::Title};
use crate::{imdb::storage::Storage, Res};
use log::info;
use std::{io::Read, path::Path};

pub struct Service {
  basics_db: Basics,
}

impl Service {
  pub fn new(app_cache_dir: &Path) -> Res<Self> {
    info!("Loading IMDB Databases...");
    let storage = Storage::load_db_files(app_cache_dir)?;

    let basics_db = Basics::new(storage.basics_db_buf.bytes())?;
    info!("Done loading IMDB Basics DB");

    Ok(Service { basics_db })
  }

  pub fn get_movie(
    &self,
    title: &str,
    year: Option<u16>,
  ) -> Option<impl Iterator<Item = &Title>> {
    self.basics_db.lookup_movie(title, year)
  }
}
