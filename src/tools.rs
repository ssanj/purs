use std::collections::HashMap;
use std::hash::Hash;

pub fn group_by<K, V, F, I>(values: I, f: F) -> HashMap<K, Vec<V>>
  where F: Fn(&V) -> K,
        K: Eq + Hash,
        I: IntoIterator<Item = V>
  {
      let mut map = HashMap::new();
      values.into_iter().for_each(|v|{
        let k = f(&v);
        map.entry(k).or_insert(vec![]).push(v)
      });

      map
  }
