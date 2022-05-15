use std::collections::HashMap;
use std::hash::Hash;

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn partition<T, E, I>(results: I) -> (Vec<T>, Vec<E>)
  where I: IntoIterator<Item = Result<T, E>>
{
  let mut errors: Vec<E> = vec![];
  let mut successes: Vec<T> = vec![];

  results.into_iter().for_each(|r| match r {
    Ok(value) => successes.push(value),
    Err(error) => errors.push(error),
  });

  (successes, errors)
}
