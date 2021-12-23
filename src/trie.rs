#![warn(clippy::all)]

use fnv::FnvHashMap;
use std::{hash::Hash, iter::Peekable};

#[derive(Default)]
pub struct Trie<K, V> {
  values: Vec<V>,
  next: FnvHashMap<K, Self>,
}

impl<K, V> Trie<K, V> {
  fn add_value(&mut self, value: V) {
    self.values.push(value);
  }
}

impl<K: Eq + Hash + Default, V: Default> Trie<K, V> {
  pub fn insert(&mut self, key: &mut impl Iterator<Item = K>, value: V) {
    match key.next() {
      Some(c) => self.next.entry(c).or_insert_with(Trie::default).insert(key, value),
      None => self.add_value(value),
    }
  }
}

impl<K: Eq + Hash, V> Trie<K, V> {
  pub fn lookup_exact(&self, keyword: &mut impl Iterator<Item = K>) -> Option<impl Iterator<Item = &V>> {
    fn helper<'a, K: Eq + Hash, V>(
      trie: &'a Trie<K, V>,
      keyword: &mut impl Iterator<Item = K>,
    ) -> Option<std::slice::Iter<'a, V>> {
      if let Some(c) = keyword.next() {
        let next_trie = trie.next.get(&c)?;
        return helper(next_trie, keyword);
      }

      if trie.values.is_empty() {
        None
      } else {
        Some(trie.values.iter())
      }
    }

    helper(self, keyword)
  }

  pub fn lookup_keyword(&self, keyword: &mut (impl Iterator<Item = K> + Clone)) -> Vec<&V> {
    fn helper<'a, K: Eq + Hash, V>(
      trie: &'a Trie<K, V>,
      original_keyword: impl Iterator<Item = K> + Clone,
      keyword: &mut Peekable<impl Iterator<Item = K>>,
      res: &mut Vec<&'a V>,
    ) {
      if keyword.peek().is_none() {
        res.extend(trie.all_values());
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
}

impl<K, V> Trie<K, V> {
  fn all_values(&self) -> Vec<&V> {
    fn helper<'a, K, V>(trie: &'a Trie<K, V>, res: &mut Vec<&'a V>) {
      res.extend(&trie.values);

      for next in trie.next.values() {
        helper(next, res);
      }
    }

    let mut res = vec![];
    helper(self, &mut res);
    res
  }
}

#[cfg(test)]
impl<K: Eq + Hash, V: Copy> Trie<K, V> {
  pub fn lookup_exact_as_vec(&self, keyword: &mut impl Iterator<Item = K>) -> Option<Vec<V>> {
    self.lookup_exact(keyword).map(|i| i.copied().collect())
  }
}

#[cfg(test)]
impl<K: Eq + Hash, V: Copy + Ord> Trie<K, V> {
  pub fn lookup_keyword_nonref(&self, keyword: &mut (impl Iterator<Item = K> + Clone)) -> Vec<V> {
    let mut res: Vec<_> = self.lookup_keyword(keyword).into_iter().copied().collect();
    res.sort_unstable();
    res
  }
}

#[cfg(test)]
impl<K, V: Copy + Ord> Trie<K, V> {
  pub fn all_values_nonref(&self) -> Vec<V> {
    let mut res: Vec<_> = self.all_values().into_iter().copied().collect();
    res.sort_unstable();
    res
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_trie() -> Trie<char, usize> {
    let mut trie = Trie::default();
    trie.insert(&mut "hello world".chars(), 1);
    trie.insert(&mut "hello tvrank".chars(), 2);
    trie.insert(&mut "hello tvrank".chars(), 3);
    trie.insert(&mut "bye bye".chars(), 4);
    trie.insert(&mut "bye tvrank bye".chars(), 5);
    trie
  }

  #[test]
  fn lookup_exact() {
    let trie = make_trie();
    assert_eq!(trie.lookup_exact_as_vec(&mut "hello".chars()), None);
    assert_eq!(trie.lookup_exact_as_vec(&mut "world".chars()), None);
    assert_eq!(trie.lookup_exact_as_vec(&mut "hello world".chars()), Some(vec![1]));
    assert_eq!(trie.lookup_exact_as_vec(&mut "hello tvrank".chars()), Some(vec![2, 3]));
    assert_eq!(trie.lookup_exact_as_vec(&mut "tvrank".chars()), None);
    assert_eq!(trie.lookup_exact_as_vec(&mut "bye".chars()), None);
    assert_eq!(trie.lookup_exact_as_vec(&mut "bye bye".chars()), Some(vec![4]));
    assert_eq!(trie.lookup_exact_as_vec(&mut "bye tvrank bye".chars()), Some(vec![5]));
  }

  #[test]
  fn all_values() {
    let trie = make_trie();
    assert_eq!(trie.all_values_nonref(), vec![1, 2, 3, 4, 5]);
  }

  #[test]
  fn lookup_keyword() {
    let trie = make_trie();
    assert_eq!(trie.lookup_keyword_nonref(&mut "hello".chars()), vec![1, 2, 3]);
    assert_eq!(trie.lookup_keyword_nonref(&mut "world".chars()), vec![1]);
    assert_eq!(trie.lookup_keyword_nonref(&mut "bye".chars()), vec![4, 5]);
    assert_eq!(trie.lookup_keyword_nonref(&mut "tvrank".chars()), vec![2, 3, 5]);
    assert_eq!(trie.lookup_keyword_nonref(&mut "notexist".chars()), vec![]);
  }
}
