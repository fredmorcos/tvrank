#![warn(clippy::all)]

use crate::imdb::error::Err;
use crate::imdb::utils::tokens;
use atoi::atoi;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy)]
pub struct TitleId<'a> {
  bytes: &'a [u8],
  num: usize,
}

impl PartialEq for TitleId<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.num == other.num
  }
}

impl Eq for TitleId<'_> {}

impl Hash for TitleId<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.num.hash(state);
  }
}

impl<'a> TitleId<'a> {
  pub(crate) fn as_bytes(&self) -> &'a [u8] {
    self.bytes
  }

  pub(crate) fn as_str(&self) -> &'a str {
    unsafe { std::str::from_utf8_unchecked(self.bytes) }
  }

  pub(crate) fn as_usize(&self) -> usize {
    self.num
  }
}

impl<'a> TryFrom<&'a [u8]> for TitleId<'a> {
  type Error = Box<dyn Error>;

  fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
    if &bytes[0..2] != tokens::TT {
      return Err::id(unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned());
    }

    let num = atoi::<usize>(&bytes[2..])
      .ok_or_else(|| Err::IdNumber(unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned()))?;

    Ok(TitleId { bytes, num })
  }
}

impl fmt::Display for TitleId<'_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.as_str())
  }
}
