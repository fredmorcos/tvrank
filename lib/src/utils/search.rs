//! A string type used to ensure that search keywords are lowercase and non-empty.

pub use self::Error as SearchStringError;

/// Error type for search string construction.
#[derive(Debug, thiserror::Error)]
#[error("Search string error")]
pub enum Error {
  /// Thrown if a search string is empty.
  #[error("Search title or keyword is empty")]
  IsEmpty,
}

/// A string type used to ensure that search keywords are lowercase and non-empty.
pub struct SearchString {
  contents: String,
}

impl SearchString {
  /// Return the search string as a string slice.
  pub fn as_str(&self) -> &str {
    &self.contents
  }
}

impl AsRef<[u8]> for SearchString {
  fn as_ref(&self) -> &[u8] {
    self.contents.as_bytes()
  }
}

impl From<SearchString> for String {
  fn from(searchstring: SearchString) -> Self {
    searchstring.contents
  }
}

impl TryFrom<&str> for SearchString {
  type Error = Error;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    if value.is_empty() {
      return Err(Error::IsEmpty);
    }

    Ok(Self { contents: value.to_lowercase() })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[should_panic]
  fn empty() {
    let _ = SearchString::try_from("").unwrap();
  }

  #[test]
  fn lowercase() {
    let value = "hello";
    let search_string = SearchString::try_from(value).unwrap();
    assert_eq!(value, search_string.as_str());
  }

  #[test]
  fn non_lowercase() {
    let value = "HeLLo";
    let search_string = SearchString::try_from(value).unwrap();
    assert_eq!(value.to_lowercase(), search_string.as_str());
  }
}
