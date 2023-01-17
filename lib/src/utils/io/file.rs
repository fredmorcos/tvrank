#![warn(clippy::all)]

//! Helpers for file handling.

use std::fs::{self, File};
use std::io::{self, BufWriter};
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
pub fn open_existing(path: &Path) -> Res<Option<File>> {
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
pub fn older_than(file: &Option<File>, duration: Duration) -> bool {
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

/// Reads the contents of a file into a leaked static buffer.
///
/// # Arguments
///
/// * `file` - The file path to read.
pub fn read_static(filename: &Path) -> Res<&'static [u8]> {
  Ok(Box::leak(fs::read(filename)?.into_boxed_slice()))
}

/// Creates a file and returns a buffered writer handle to it.
///
/// # Arguments
///
/// * `file` - The file path to create.
pub fn create_buffered(filename: &Path) -> Res<BufWriter<File>> {
  Ok(BufWriter::new(File::create(filename)?))
}
