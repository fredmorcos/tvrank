#![warn(clippy::all)]

use fnv::FnvHashMap;
use std::hash::Hash;
use std::iter::Peekable;

#[derive(Default)]
pub struct Trie<K, V> {
  value: Option<V>,
  next: FnvHashMap<K, Self>,
}

impl<K: Eq + Hash + Default, V: Default> Trie<K, V> {
  pub fn insert(&mut self, key: &mut impl Iterator<Item = K>) -> &mut V {
    match key.next() {
      Some(c) => self.next.entry(c).or_insert_with(Trie::default).insert(key),
      None => self.value.get_or_insert_with(Default::default),
    }
  }
}

impl<K: Eq + Hash, V> Trie<K, V> {
  pub fn lookup_exact(&self, key: &mut impl Iterator<Item = K>) -> Option<&V> {
    let mut trie = self;
    for c in key {
      trie = trie.next.get(&c)?;
    }
    trie.value.as_ref()
  }

  pub fn lookup_keyword<'a>(&'a self, keyword: &mut (impl Iterator<Item = K> + Clone)) -> Vec<&V> {
    fn helper<'a, K: Eq + Hash, V>(
      trie: &'a Trie<K, V>,
      original_keyword: impl Iterator<Item = K> + Clone,
      keyword: &mut Peekable<impl Iterator<Item = K>>,
      res: &mut Vec<&'a V>,
    ) {
      if keyword.peek().is_none() {
        res.extend(trie.values());
        return;
      }

      let c = keyword.next().unwrap();

      match trie.next.get(&c) {
        Some(next_trie) => helper(next_trie, original_keyword, keyword, res),
        None => {
          for next_trie in trie.next.values() {
            helper(next_trie, original_keyword.clone(), &mut original_keyword.clone().peekable(), res);
          }
        }
      }
    }

    let mut res = vec![];
    helper(self, keyword.clone(), &mut keyword.peekable(), &mut res);
    res
  }

  pub fn values(&self) -> Values<K, V> {
    Values::new(self)
  }
}


pub struct Values<'a, K, V> {
  stack: Vec<&'a Trie<K, V>>,
}

impl<'a, K, V> Values<'a, K, V> {
  fn new(node: &'a Trie<K, V>) -> Self {
    Self { stack: vec![node] }
  }
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
  type Item = &'a V;

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      let trie = self.stack.pop()?;
      self.stack.extend(trie.next.values());
      if trie.value.is_some() {
        return trie.value.as_ref();
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_trie() -> Trie<char, Vec<usize>> {
    let mut trie: Trie<_, Vec<_>> = Trie::default();
    trie.insert(&mut "hello world".chars()).push(1);
    trie.insert(&mut "hello tvrank".chars()).push(2);
    trie.insert(&mut "hello tvrank".chars()).push(3);
    trie.insert(&mut "bye bye".chars()).push(4);
    trie.insert(&mut "bye tvrank bye".chars()).push(5);
    trie
  }

  #[test]
  fn lookup_exact() {
    let trie = make_trie();
    assert_eq!(trie.lookup_exact(&mut "hello".chars()), None);
    assert_eq!(trie.lookup_exact(&mut "world".chars()), None);
    assert_eq!(trie.lookup_exact(&mut "hello world".chars()), Some(&vec![1]));
    assert_eq!(trie.lookup_exact(&mut "hello tvrank".chars()), Some(&vec![2, 3]));
    assert_eq!(trie.lookup_exact(&mut "tvrank".chars()), None);
    assert_eq!(trie.lookup_exact(&mut "bye".chars()), None);
    assert_eq!(trie.lookup_exact(&mut "bye bye".chars()), Some(&vec![4]));
    assert_eq!(trie.lookup_exact(&mut "bye tvrank bye".chars()), Some(&vec![5]));
    assert_eq!(trie.lookup_exact(&mut "notexist".chars()), None);
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
    assert_eq!(sort_results!(trie.lookup_keyword(&mut "hello".chars()).into_iter()), vec![1, 2, 3]);
    assert_eq!(sort_results!(trie.lookup_keyword(&mut "world".chars()).into_iter()), vec![1]);
    assert_eq!(sort_results!(trie.lookup_keyword(&mut "bye".chars()).into_iter()), vec![4, 5]);
    assert_eq!(sort_results!(trie.lookup_keyword(&mut "tvrank".chars()).into_iter()), vec![2, 3, 5]);
    assert_eq!(sort_results!(trie.lookup_keyword(&mut "notexist".chars()).into_iter()), vec![]);
  }
}
