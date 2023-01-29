#![warn(clippy::all)]

use crate::imdb::title_type::TitleType;
use crate::utils::result::Res;
use derive_more::Display;
use std::error::Error;

/// Error types of the TvRank library
#[derive(Debug, Display)]
#[display(fmt = "{}")]
pub enum Err {
  /// Thrown if an ID does not start with `tt`
  #[display(fmt = "ID `{_0}` does not start with `tt` (e.g. ttXXXXXXX)")]
  Id(String),
  /// Thrown if an ID does not contain a valid number
  #[display(fmt = "ID `{_0}` does not contain a valid number (e.g. ttXXXXXXX)")]
  IdNumber(String),
  /// Thrown if the ID already exists
  #[display(fmt = "Duplicate IMDB ID `{_0}` found")]
  DuplicateId(String),
  /// Thrown if the given title type does not exist
  #[display(fmt = "Unknown title type")]
  TitleType,
  /// Thrown if the adult marker is invalid
  #[display(fmt = "Invalid adult marker")]
  Adult,
  /// Thrown if the start year is not a number
  #[display(fmt = "Start year is not a number")]
  StartYear,
  /// Thrown if the end year is not a number
  #[display(fmt = "End year is not a number")]
  EndYear,
  /// Thrown if the runtime minutes is not a number
  #[display(fmt = "Runtime minutes is not a number")]
  RuntimeMinutes,
  /// Thrown if the given genre is invalid
  #[display(fmt = "Invalid genre")]
  Genre,
  /// Thrown if the end of file is reached
  #[display(fmt = "Unexpected end of file")]
  Eof,
  /// Thrown if the given number of votes is not a number
  #[display(fmt = "Number of votes is not a number")]
  Votes,
  /// Thrown if a problem occurs while building the IMDB basics DB
  #[display(fmt = "Error building the IMDB basics DB")]
  BasicsDbBuild,
  /// Thrown if a problem occurs while querying the IMDB basics DB
  #[display(fmt = "Error querying the IMDB basics DB")]
  BasicsDbQuery,
  /// Thrown if the given title type is not supported
  #[display(fmt = "Unsupported title type `{_0}`")]
  UnsupportedTitleType(TitleType),
  /// Thrown if a problem occurs while parsing a title
  #[display(fmt = "Error parsing title: {_0}")]
  ParsingTitle(String),
}

impl Err {
  /// Returns a Result containing an ID error
  pub(crate) fn id<T>(id: String) -> Res<T> {
    Err(Box::new(Err::Id(id)))
  }

  /// Returns a Result containing an ID Number error
  pub(crate) fn id_number<T>(id: String) -> Res<T> {
    Err(Box::new(Err::IdNumber(id)))
  }

  /// Returns a Result containing a DuplicateId error with the given ID
  pub(crate) fn duplicate_id<T>(id: String) -> Res<T> {
    Err(Box::new(Err::DuplicateId(id)))
  }

  /// Returns a Result containing an Adult error
  pub(crate) fn adult<T>() -> Res<T> {
    Err(Box::new(Err::Adult))
  }

  /// Returns a Result containing an Eof error inside
  pub(crate) fn eof<T>() -> Res<T> {
    Err(Box::new(Err::Eof))
  }

  /// Returns a Result containing an UnsupportedTitleType error with the given TitleType
  pub(crate) fn unsupported_title_type<T>(title_type: TitleType) -> Res<T> {
    Err(Box::new(Err::UnsupportedTitleType(title_type)))
  }
}

impl Error for Err {}
