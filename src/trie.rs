#![warn(clippy::all)]

use fnv::FnvHashMap;
use iter::Children;
use iter::Values;

#[derive(Default)]
pub struct Trie<V> {
  value: Option<V>,
  next: FnvHashMap<char, Self>,
}

impl<V> Trie<V> {
  fn child(&self, k: &char) -> Option<&Trie<V>> {
    self.next.get(k)
  }

  fn children(&self) -> Children<V> {
    Children::new(self)
  }
}

impl<V: Default> Trie<V> {
  fn child_or_default(&mut self, k: char) -> &mut Trie<V> {
    self.next.entry(k).or_insert_with(Default::default)
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
      trie = trie.child(&c)?;
    }
    trie.value.as_ref()
  }

  pub fn lookup_keyword(&self, keyword: &str) -> Vec<&V> {
    fn helper<'a, V>(
      trie: &'a Trie<V>,
      original_keyword: &str,
      mut keyword: impl Iterator<Item = char>,
      res: &mut Vec<&'a V>,
    ) {
      if let Some(c) = keyword.next() {
        if let Some(next_trie) = trie.child(&c) {
          helper(next_trie, original_keyword, keyword, res);
        } else {
          for next_trie in trie.children() {
            helper(next_trie, original_keyword, original_keyword.chars(), res);
          }
        }
      } else {
        res.extend(trie.values());
      }
    }

    let mut res = vec![];
    helper(self, keyword, keyword.chars(), &mut res);
    res
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

    pub(crate) fn new_empty() -> Self {
      Self { stack: vec![] }
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

  pub struct Children<'a, V> {
    iter: HashMapValues<'a, char, Trie<V>>,
  }

  impl<'a, V> Children<'a, V> {
    pub(crate) fn new(node: &'a Trie<V>) -> Self {
      Self { iter: node.next.values() }
    }
  }

  impl<'a, V> Iterator for Children<'a, V> {
    type Item = &'a Trie<V>;

    fn next(&mut self) -> Option<Self::Item> {
      self.iter.next()
    }
  }
}

mod exts {
  pub trait CharExt {
    fn is_skippable(&self) -> bool;
  }

  impl CharExt for char {
    fn is_skippable(&self) -> bool {
      *self == '-' || *self == ':' || *self == '\'' || self.is_whitespace()
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_trie() -> Trie<Vec<usize>> {
    let mut trie: Trie<Vec<_>> = Trie::default();
    trie.insert("hello world").push(1);
    trie.insert("hello tvrank").push(2);
    trie.insert("hello tvrank").push(3);
    trie.insert("bye bye").push(4);
    trie.insert("bye tvrank bye").push(5);
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
    assert_eq!(sort_results!(trie.values()), vec![1, 2, 3, 4, 5]);
  }

  #[test]
  fn lookup_keyword() {
    let trie = make_trie();
    assert_eq!(sort_results!(trie.lookup_keyword("hello").into_iter()), vec![1, 2, 3]);
    assert_eq!(sort_results!(trie.lookup_keyword("world").into_iter()), vec![1]);
    assert_eq!(sort_results!(trie.lookup_keyword("bye").into_iter()), vec![4, 5]);
    assert_eq!(sort_results!(trie.lookup_keyword("tvrank").into_iter()), vec![2, 3, 5]);
    assert_eq!(sort_results!(trie.lookup_keyword("notexist").into_iter()), vec![]);
    assert_eq!(sort_results!(trie.lookup_keyword("hellofoo").into_iter()), vec![]);
    assert_eq!(sort_results!(trie.lookup_keyword("byefoo").into_iter()), vec![]);
  }
}
