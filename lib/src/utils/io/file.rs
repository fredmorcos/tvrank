#![warn(clippy::all)]

//! Helpers for file handling.

use std::fs::File;
use std::io;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::utils::result::Res;

/// Returns the file at the given path if it exists, or an Ok Result if it is not found.
///
/// Only returns an error if a problem occurs while opening an existing file.
///
/// # Arguments
///
/// * `path` - Path of the file to be opened.
pub fn file_exists(path: &Path) -> Res<Option<File>> {
  match File::open(path) {
    Ok(f) => Ok(Some(f)),
    Err(e) => match e.kind() {
      io::ErrorKind::NotFound => Ok(None),
      _ => Err(Box::new(e)),
    },
  }
}

/// Determines if the given database file is old.
///
/// # Arguments
///
/// * `file` - Database file to be checked.
/// * `duration` - The duration by which the file would be considered old.
pub fn file_older_than(file: &Option<File>, duration: Duration) -> bool {
  if let Some(f) = file {
    if let Ok(md) = f.metadata() {
      if let Ok(modified) = md.modified() {
        match SystemTime::now().duration_since(modified) {
          Ok(age) => return age >= duration,
          Err(_) => return true,
        }
      }
    }
  }

  // The file does not exist or its metadata or modification date could not be read.
  true
}
