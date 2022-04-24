#![warn(clippy::all)]

use crate::error::Err;
use crate::tokens;
use atoi::FromRadix10;
use serde::{Serialize, Serializer};
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};

/// The ID corresponding to a title as u8 and usize
#[derive(Debug, Clone, Copy)]
pub struct TitleId<'storage> {
  bytes: &'storage [u8],
  num: usize,
}

impl<'storage> Serialize for TitleId<'storage> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(unsafe { std::str::from_utf8_unchecked(self.bytes) })
  }
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

impl<'storage> TitleId<'storage> {
  /// Returns the title id as bytes
  pub(crate) fn as_bytes(&self) -> &'storage [u8] {
    self.bytes
  }

  /// Returns the title id as str
  pub(crate) fn as_str(&self) -> &'storage str {
    unsafe { std::str::from_utf8_unchecked(self.bytes) }
  }

  /// Returns the title id as usize
  pub(crate) fn as_usize(&self) -> usize {
    self.num
  }
}

impl<'storage> TryFrom<&'storage [u8]> for TitleId<'storage> {
  type Error = Box<dyn Error>;

  fn try_from(bytes: &'storage [u8]) -> Result<Self, Self::Error> {
    if &bytes[0..2] != tokens::TT {
      return Err::id(unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned());
    }

    let num = &bytes[2..];
    let num_len = num.len();
    let num = match usize::from_radix_10(num) {
      (val, len) if len == num_len => val,
      _ => return Err::id_number(unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned()),
    };

    Ok(TitleId { bytes, num })
  }
}

impl<'a> TryFrom<&'a str> for TitleId<'a> {
  type Error = Box<dyn Error>;

  fn try_from(id: &'a str) -> Result<Self, Self::Error> {
    TitleId::try_from(id.as_bytes())
  }
}

impl fmt::Display for TitleId<'_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.as_str())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test() {
    let id = TitleId::try_from("tt0000001".as_bytes()).unwrap();
    assert_eq!(id.as_bytes(), "tt0000001".as_bytes());
    assert_eq!(id.as_str(), "tt0000001");
    assert_eq!(id.as_usize(), 1);
  }
}
