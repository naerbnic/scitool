use std::collections::{BTreeMap, btree_map};

mod token {
    pub(crate) struct SealingToken {}
}

pub(crate) fn eq_unordered<I1, I2>(i1: I1, i2: I2) -> bool
where
    I1: IntoIterator,
    I2: IntoIterator<Item = I1::Item>,
    I1::Item: PartialEq<I2::Item> + Ord,
{
    let mut item_set: BTreeMap<_, usize> = BTreeMap::new();
    for item in i1 {
        *item_set.entry(item).or_insert(0) += 1;
    }
    for item in i2 {
        match item_set.entry(item) {
            btree_map::Entry::Vacant(_) => return false,
            btree_map::Entry::Occupied(mut occ) => {
                let count = occ.get_mut();
                *count -= 1;
                if *count == 0 {
                    occ.remove();
                }
            }
        }
    }
    item_set.is_empty()
}

pub(crate) trait IterExt: Iterator {
    /// For an iterator that returns `Result<T, E>`, extract the error into a `Result`, with the
    /// value being an iterator of the successful values.
    ///
    /// This materializes the iterator.
    fn extract_err<T, E>(self) -> Result<impl Iterator<Item = T>, E>
    where
        Self: Iterator<Item = Result<T, E>> + Sized,
    {
        Ok(self.collect::<Result<Vec<T>, E>>()?.into_iter())
    }

    #[doc(hidden)]
    #[expect(dead_code, reason = "used for sealing")]
    fn sealed_result(_: token::SealingToken);
}

impl<T> IterExt for T
where
    T: Iterator,
{
    #[doc(hidden)]
    fn sealed_result(_: token::SealingToken) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("test error")]
    struct TestError;

    #[test]
    fn test_eq_unordered() {
        assert!(eq_unordered([1, 2, 3], [3, 2, 1]));
        assert!(!eq_unordered([1, 2, 3], [3, 2, 2]));
    }

    #[test]
    fn test_extract_err() -> anyhow::Result<()> {
        let result: Vec<_> = vec![0, 1, 2]
            .into_iter()
            .map(|i: u8| i.checked_div(2).ok_or(TestError))
            .extract_err()?
            .map(|i| i * 2)
            .collect();
        assert_eq!(result, vec![0, 0, 2]);
        Ok(())
    }
}
