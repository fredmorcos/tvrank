#![warn(clippy::all)]

use crate::imdb::error::Err;
use crate::imdb::utils::tokens;
use atoi::atoi;
use serde::{Serialize, Serializer};
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};

/// The ID corresponding to a title as u8 and usize
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TitleId<'a> {
  #[serde(serialize_with = "bytes_serializer", rename = "title_id")]
  bytes: &'a [u8],

  #[serde(skip_serializing)]
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
  /// Returns the title id as bytes
  pub(crate) fn as_bytes(&self) -> &'a [u8] {
    self.bytes
  }

  /// Returns the title id as str
  pub(crate) fn as_str(&self) -> &'a str {
    unsafe { std::str::from_utf8_unchecked(self.bytes) }
  }

  /// Returns the title id as usize
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

fn bytes_serializer<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  let s = unsafe { std::str::from_utf8_unchecked(bytes) };

  serializer.serialize_str(s)
}
