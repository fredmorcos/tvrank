#![warn(clippy::all)]

use super::title::TitleId;
use crate::Res;
use derive_more::Display;
use std::error::Error;

#[derive(Debug, Display)]
#[display(fmt = "{}")]
pub enum Err {
  #[display(fmt = "ID does not start with `tt` (e.g. ttXXXXXXX)")]
  Id,
  #[display(fmt = "ID does not contain a number (e.g. ttXXXXXXX)")]
  IdNumber,
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
  #[display(fmt = "Duplicate IMDB ID: {}", _0)]
  Duplicate(TitleId),
  #[display(fmt = "Number of votes is not a number")]
  Votes,
}

impl Err {
  pub(crate) fn id<T>() -> Res<T> {
    Err(Box::new(Err::Id))
  }

  pub(crate) fn adult<T>() -> Res<T> {
    Err(Box::new(Err::Adult))
  }

  pub(crate) fn duplicate<T>(id: TitleId) -> Res<T> {
    Err(Box::new(Err::Duplicate(id)))
  }
}

impl Error for Err {}
