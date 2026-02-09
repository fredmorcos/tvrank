#![warn(clippy::all)]

//! Module for handling title information objects.

use std::fs;
use std::io::{self, BufReader};
use std::path::Path;

use crate::imdb::ImdbTitleId;

use log::warn;
use serde::{Deserialize, Deserializer, Serialize};

/// Errors when parsing title information.
#[derive(Debug, thiserror::Error)]
#[error("Error parsing title information file")]
pub enum Error {
  /// JSON errors (e.g. when the title information file is incorrect).
  #[error("Error parsing title info JSON file: {0}")]
  Json(#[from] serde_json::Error),
  /// IO errors.
  #[error("IO error: {0}")]
  Io(#[from] io::Error),
}

/// The IMDB section of title information files.
#[derive(Serialize, Deserialize)]
pub struct ImdbTitleInfo<'a> {
  #[serde(deserialize_with = "deserialize_titleid")]
  id: ImdbTitleId<'a>,
}

impl<'a> ImdbTitleInfo<'a> {
  /// Construct an IMDB title information object from an IMDB ID.
  pub fn new(id: ImdbTitleId<'a>) -> Self {
    Self { id }
  }

  /// Get the IMDB ID of an IMDB title information object.
  pub fn id(&'_ self) -> &'_ ImdbTitleId<'_> {
    &self.id
  }
}

fn deserialize_titleid<'a, 'de, D>(deserializer: D) -> Result<ImdbTitleId<'a>, D::Error>
where
  D: Deserializer<'de>,
{
  let s = Box::leak(String::deserialize(deserializer)?.into_boxed_str());
  let id = ImdbTitleId::try_from(s.as_bytes()).map_err(serde::de::Error::custom)?;
  Ok(id)
}

/// Title information.
///
/// This is primarily used when scanning directories, where some titles may have the same
/// name and have been released during the same year, making lookup only by directory name
/// ambiguous. This structure is used to represent a file on disk which contains JSON data
/// which can uniquely identify the title.
#[derive(Serialize, Deserialize)]
pub struct TitleInfo<'a> {
  imdb: ImdbTitleInfo<'a>,
}

impl<'a> TitleInfo<'a> {
  /// Construct a title information object from an IMDB ID.
  pub fn new(id: ImdbTitleId<'a>) -> Self {
    Self { imdb: ImdbTitleInfo::new(id) }
  }

  /// Load a title information object from a file.
  pub fn from_path(path: &'_ Path) -> Result<TitleInfo<'_>, Error> {
    let title_info_path = path.join("tvrank.json");
    let title_info_file = fs::File::open(&title_info_path)?;
    let title_info_file_reader = BufReader::new(title_info_file);
    let title_info: Result<TitleInfo, _> = serde_json::from_reader(title_info_file_reader);

    let title_info = match title_info {
      Ok(title_info) => title_info,
      Err(err) => {
        warn!("Ignoring info in `{}` due to parse error: {}", title_info_path.display(), err);
        return Err(Error::Json(err));
      }
    };

    Ok(title_info)
  }

  /// Get the IMDB title information object from a top-level title information object.
  pub fn imdb(&'_ self) -> &'_ ImdbTitleInfo<'_> {
    &self.imdb
  }
}
