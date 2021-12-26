#![warn(clippy::all)]

use self::iter::Children;
use self::iter::KeywordValues;
use self::iter::Matches;
use self::iter::Values;
use fnv::FnvHashMap;

#[derive(PartialEq, Eq)]
pub struct Trie<V> {
  value: Option<V>,
  next_ascii: Box<[Option<Self>; 95]>,
  next: FnvHashMap<char, Self>,
}

impl<V> Default for Trie<V> {
  fn default() -> Self {
    Self {
      value: Default::default(),
      next_ascii: Box::new([
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None,
      ]),
      next: Default::default(),
    }
  }
}

impl<V> Trie<V> {
  fn char_is_ascii(c: char) -> bool {
    c.is_ascii_graphic() || c == ' '
  }

  fn index_from_char(c: char) -> usize {
    (c as usize) - 32
  }

  fn child(&self, k: char) -> Option<&Trie<V>> {
    if Self::char_is_ascii(k) {
      unsafe { self.next_ascii.get_unchecked(Self::index_from_char(k)) }.as_ref()
    } else {
      self.next.get(&k)
    }
  }

  fn children(&self) -> Children<V> {
    Children::new(self)
  }

  fn matches<'a, 'k>(&'a self, keyword: &'k str) -> Matches<'a, 'k, V> {
    Matches::new(self, keyword)
  }
}

impl<V: Default> Trie<V> {
  fn child_or_default(&mut self, k: char) -> &mut Trie<V> {
    if Self::char_is_ascii(k) {
      let cell = unsafe { self.next_ascii.get_unchecked_mut(Self::index_from_char(k)) };
      cell.get_or_insert_with(Default::default)
    } else {
      self.next.entry(k).or_insert_with(Default::default)
    }
  }

  pub fn insert(&mut self, key: &str) -> &mut V {
    self.insert_iter(key.chars())
  }

  fn insert_iter(&mut self, key: impl Iterator<Item = char>) -> &mut V {
    let mut trie = self;
    for c in key {
      trie = trie.child_or_default(c);
    }
    trie.value.get_or_insert_with(Default::default)
  }
}

impl<V> Trie<V> {
  pub fn lookup_exact(&self, key: &str) -> Option<&V> {
    self.lookup_exact_iter(key.chars())
  }

  fn lookup_exact_iter(&self, key: impl Iterator<Item = char>) -> Option<&V> {
    let mut trie = self;
    for c in key {
      trie = trie.child(c)?;
    }
    trie.value.as_ref()
  }

  pub fn lookup_keyword<'k>(&self, keyword: &'k str) -> KeywordValues<'_, 'k, V> {
    KeywordValues::new(self, keyword)
  }

  pub fn values(&self) -> Values<V> {
    Values::new(self)
  }
}

mod iter {
  use super::Trie;

  pub struct Values<'a, V> {
    stack: Vec<&'a Trie<V>>,
  }

  impl<'a, V> Values<'a, V> {
    pub(crate) fn new(node: &'a Trie<V>) -> Self {
      Self { stack: vec![node] }
    }

    pub(crate) fn empty() -> Self {
      Self { stack: vec![] }
    }

    pub(crate) fn placeholder() -> Self {
      Self::empty()
    }
  }

  impl<'a, V> Iterator for Values<'a, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
      loop {
        let trie = self.stack.pop()?;
        self.stack.extend(trie.children());
        if trie.value.is_some() {
          return trie.value.as_ref();
        }
      }
    }
  }

  use std::collections::hash_map::Values as HashMapValues;

  pub(crate) struct Children<'a, V> {
    iter_ascii: std::slice::Iter<'a, Option<Trie<V>>>,
    iter: HashMapValues<'a, char, Trie<V>>,
  }

  impl<'a, V> Children<'a, V> {
    pub(crate) fn new(node: &'a Trie<V>) -> Self {
      Self { iter_ascii: node.next_ascii.iter(), iter: node.next.values() }
    }
  }

  impl<'a, V> Iterator for Children<'a, V> {
    type Item = &'a Trie<V>;

    fn next(&mut self) -> Option<Self::Item> {
      'LOOP: loop {
        match self.iter_ascii.next() {
          Some(Some(next_trie)) => return Some(next_trie),
          Some(None) => continue 'LOOP,
          None => return self.iter.next(),
        }
      }
    }
  }

  pub struct KeywordValues<'a, 'k, V> {
    stack: Vec<&'a Trie<V>>,
    keyword: &'k str,
    values: Values<'a, V>,
    matches: Vec<(&'a Trie<V>, &'a Trie<V>)>,
  }

  impl<'a, 'k, V> KeywordValues<'a, 'k, V> {
    pub fn new(node: &'a Trie<V>, keyword: &'k str) -> Self {
      Self { stack: vec![node], keyword, values: Values::placeholder(), matches: vec![] }
    }
  }

  impl<'a, 'k, V: PartialEq> Iterator for KeywordValues<'a, 'k, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
      if let Some(value) = self.values.next() {
        return Some(value);
      }

      if let Some((_, end)) = self.matches.pop() {
        self.values = end.values();
        return self.next();
      };

      loop {
        let anchor = self.stack.pop()?;
        self.matches.extend(anchor.matches(self.keyword));
        if self.matches.is_empty() {
          self.stack.extend(anchor.children());
        } else {
          self.stack.extend(anchor.children().filter(|&t| {
            for (start, _) in &self.matches {
              if t == *start {
                return false;
              }
            }

            true
          }));
          return self.next();
        };
      }
    }
  }

  struct MatchState<'a, 'k, V> {
    start: bool,
    start_trie: &'a Trie<V>,
    current_trie: &'a Trie<V>,
    keyword: std::str::Chars<'k>,
  }

  impl<'a, 'k, V> MatchState<'a, 'k, V> {
    fn new(
      start: bool,
      start_trie: &'a Trie<V>,
      current_trie: &'a Trie<V>,
      keyword: std::str::Chars<'k>,
    ) -> Self {
      Self { start, start_trie, current_trie, keyword }
    }
  }

  pub(crate) struct Matches<'a, 'k, V> {
    stack: Vec<MatchState<'a, 'k, V>>,
  }

  impl<'a, 'k, V> Matches<'a, 'k, V> {
    pub(crate) fn new(node: &'a Trie<V>, keyword: &'k str) -> Self {
      Self { stack: vec![MatchState::new(true, node, node, keyword.chars())] }
    }
  }

  impl<'a, 'k, V> Iterator for Matches<'a, 'k, V> {
    type Item = (&'a Trie<V>, &'a Trie<V>);

    fn next(&mut self) -> Option<Self::Item> {
      let mut state = self.stack.pop()?;

      while let Some(c) = state.keyword.next() {
        let found = if let Some(next_trie) = state.current_trie.child(c) {
          if state.start {
            state.start_trie = next_trie;
          }

          state.current_trie = next_trie;
          state.start = false;
          true
        } else {
          false
        };

        for skippable in ['-', ':', '\''] {
          if let Some(next_trie) = state.current_trie.child(skippable) {
            let start = state.start_trie;
            let keyword = state.keyword.clone();
            let mut new_state = MatchState::new(false, start, next_trie, keyword);

            if state.start {
              new_state.start_trie = next_trie;
            }

            self.stack.push(new_state);
          }
        }

        if !found {
          return self.next();
        }
      }

      Some((state.start_trie, state.current_trie))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Trie;

  fn make_trie() -> Trie<Vec<usize>> {
    let mut trie: Trie<Vec<_>> = Trie::default();
    trie.insert("hello world").push(1);
    trie.insert("hello tvrank").push(2);
    trie.insert("hello tvrank").push(3);
    trie.insert("bye bye").push(4);
    trie.insert("bye tvrank bye").push(5);
    trie.insert("hello tvrank bye").push(6);
    trie.insert("spider-man").push(7);
    trie.insert("spiderman").push(8);
    trie.insert("él spidér-man").push(9);
    trie
  }

  #[test]
  fn lookup_exact() {
    let trie = make_trie();
    assert_eq!(trie.lookup_exact("hello"), None);
    assert_eq!(trie.lookup_exact("world"), None);
    assert_eq!(trie.lookup_exact("hello world"), Some(&vec![1]));
    assert_eq!(trie.lookup_exact("hello tvrank"), Some(&vec![2, 3]));
    assert_eq!(trie.lookup_exact("tvrank"), None);
    assert_eq!(trie.lookup_exact("bye"), None);
    assert_eq!(trie.lookup_exact("bye bye"), Some(&vec![4]));
    assert_eq!(trie.lookup_exact("bye tvrank bye"), Some(&vec![5]));
    assert_eq!(trie.lookup_exact("hello tvrank bye"), Some(&vec![6]));
    assert_eq!(trie.lookup_exact("notexist"), None);
  }

  macro_rules! sort_results {
    ($iter:expr) => {{
      let mut res = $iter.flatten().copied().collect::<Vec<usize>>();
      res.sort_unstable();
      res
    }};
  }

  #[test]
  fn all_values() {
    let trie = make_trie();
    assert_eq!(sort_results!(trie.values()), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
  }

  #[test]
  fn lookup_keyword() {
    let trie = make_trie();
    assert_eq!(sort_results!(trie.lookup_keyword("hello")), vec![1, 2, 3, 6]);
    assert_eq!(sort_results!(trie.lookup_keyword("world")), vec![1]);
    assert_eq!(sort_results!(trie.lookup_keyword("bye")), vec![4, 5, 6]);
    assert_eq!(sort_results!(trie.lookup_keyword("tvrank")), vec![2, 3, 5, 6]);
    assert_eq!(sort_results!(trie.lookup_keyword("notexist")), vec![]);
    assert_eq!(sort_results!(trie.lookup_keyword("hellofoo")), vec![]);
    assert_eq!(sort_results!(trie.lookup_keyword("byefoo")), vec![]);
    assert_eq!(sort_results!(trie.lookup_keyword("spider-man")), vec![7]);
    assert_eq!(sort_results!(trie.lookup_keyword("spiderman")), vec![7, 8]);
    assert_eq!(sort_results!(trie.lookup_keyword("spidér-man")), vec![9]);
    assert_eq!(sort_results!(trie.lookup_keyword("spidérman")), vec![9]);
  }

  #[test]
  fn matches() {
    let trie = make_trie();
    assert_eq!(trie.matches("spider-man").count(), 1);
    assert_eq!(trie.matches("spiderman").count(), 2);
    assert_eq!(trie.matches("él spidér-man").count(), 1);
    assert_eq!(trie.matches("él spidérman").count(), 1);
  }
}
