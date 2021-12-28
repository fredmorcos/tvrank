#![warn(clippy::all)]

use super::title::TitleId;
use crate::Res;
use derive_more::Display;
use std::error::Error;

#[derive(Debug, Display)]
#[display(fmt = "{}")]
pub enum Err {
  #[display(fmt = "ID does not start with `tt` (e.g. ttXXXXXXX)")]
  Id(String),
  #[display(fmt = "ID does not contain a number (e.g. ttXXXXXXX)")]
  IdNumber,
  #[display(fmt = "Duplicate IMDB ID `{}` found", _0)]
  DuplicateId(TitleId<'static>),
  #[display(fmt = "Unknown title type")]
  TitleType,
  #[display(fmt = "Invalid adult marker")]
  Adult,
  #[display(fmt = "Start year is not a number")]
  StartYear,
  #[display(fmt = "End year is not a number")]
  EndYear,
  #[display(fmt = "Runtime minutes is not a number")]
  RuntimeMinutes,
  #[display(fmt = "Invalid genre")]
  Genre,
  #[display(fmt = "Unexpected end of file")]
  Eof,
  #[display(fmt = "Number of votes is not a number")]
  Votes,
  #[display(fmt = "Error building the IMDB basics DB")]
  BasicsDbBuild,
  #[display(fmt = "Error querying the IMDB basics DB")]
  BasicsDbQuery,
  #[display(fmt = "Unsupported title type")]
  UnsupportedTitleType,
}

impl Err {
  pub(crate) fn id<T>(id: String) -> Res<T> {
    Err(Box::new(Err::Id(id)))
  }

  pub(crate) fn duplicate_id<T>(id: TitleId<'static>) -> Res<T> {
    Err(Box::new(Err::DuplicateId(id)))
  }

  pub(crate) fn adult<T>() -> Res<T> {
    Err(Box::new(Err::Adult))
  }

  pub(crate) fn basics_db_build<T>() -> Res<T> {
    Err(Box::new(Err::BasicsDbBuild))
  }

  pub(crate) fn basics_db_query<T>() -> Res<T> {
    Err(Box::new(Err::BasicsDbQuery))
  }
}

impl Error for Err {}
