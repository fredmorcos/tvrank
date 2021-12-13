#![warn(clippy::all)]

use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use fnv::FnvHashSet;
use std::{collections::hash_set, fmt};

#[derive(Clone)]
pub struct KeywordSet {
  keywords: FnvHashSet<String>,
  matcher: AhoCorasick,
}

impl fmt::Display for KeywordSet {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for keyword in &self.keywords {
      write!(f, "`{}` ", keyword)?
    }

    Ok(())
  }
}

impl KeywordSet {
  pub fn into_hashset_iter(self) -> hash_set::IntoIter<String> {
    self.keywords.into_iter()
  }

  pub fn hashset_iter(&self) -> hash_set::Iter<String> {
    self.keywords.iter()
  }

  pub fn len(&self) -> usize {
    self.keywords.len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn matches(&self, text: &str) -> bool {
    let mut matches: Vec<usize> = self.matcher.find_iter(text).map(|mat| mat.pattern()).collect();
    matches.sort_unstable();
    matches.dedup();
    matches.len() == self.keywords.len()
  }
}

impl TryFrom<&str> for KeywordSet {
  type Error = ();

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    let mut keywords = FnvHashSet::default();

    if value.is_empty() {
      return Err(());
    }

    for keyword in value.split_whitespace() {
      if keyword.is_empty() || keyword.len() == 1 {
        continue;
      }

      let keyword = keyword.to_lowercase();
      keywords.insert(keyword);
    }

    if keywords.is_empty() {
      return Err(());
    }

    let matcher = AhoCorasickBuilder::new().build(&keywords);

    Ok(Self { keywords, matcher })
  }
}
