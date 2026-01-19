//! Expressions, which can be used to query the map.

use std::{collections::HashSet, fmt::Debug};

use crate::collections::indexed_map::{MapStorage, StorageId};

/// The implementation of a predicate on a single index.
///
/// Indexes must implement this in order to be used in an expression.
pub(super) trait IndexPredicate<'a, T>: Debug {
    /// Returns a size hint for the number of entries in the index that match the predicate.
    fn size_hint(
        &self,
        _storage: &MapStorage<T>,
        _negation: IndexNegation,
    ) -> (usize, Option<usize>) {
        (0, None)
    }

    /// Adds all ids of entries in the index that match the predicate to the
    /// results set.
    fn find_matching(&self, storage: &MapStorage<T>, results: &mut HashSet<StorageId>);

    /// If there is an optimization that allows us to find all ids of entries
    /// in the index that do not match the predicate, returns Ok(()). Otherwise,
    /// returns Err(()).
    fn try_find_non_matching(
        &self,
        _storage: &MapStorage<T>,
        _results: &mut HashSet<StorageId>,
    ) -> Result<(), ()> {
        Err(())
    }

    /// Returns whether a specific entry matches the predicate.
    fn matches(&self, storage: &MapStorage<T>, id: StorageId) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IndexNegation {
    Negated,
    Plain,
}

impl IndexNegation {
    fn negate(self) -> Self {
        match self {
            Self::Negated => Self::Plain,
            Self::Plain => Self::Negated,
        }
    }
}

struct IndexPredBox<'a, T> {
    pred: Box<dyn IndexPredicate<'a, T> + 'a>,
    negation: IndexNegation,
}

impl<T> IndexPredBox<'_, T> {
    fn negate(mut self) -> Self {
        self.negation = self.negation.negate();
        self
    }

    fn find_matching(&self, storage: &MapStorage<T>, results: &mut HashSet<StorageId>) {
        match self.negation {
            IndexNegation::Negated => {
                if let Ok(()) = self.pred.try_find_non_matching(storage, results) {
                    // There was an optimization in this implementation, so it should
                    // have already added all non-matching ids to the results set.
                    return;
                }

                // Go the slow path: Go through each id and add it to the results set
                // if it does not match the predicate.
                for id in storage.all_ids() {
                    if !self.pred.matches(storage, id) {
                        results.insert(id);
                    }
                }
            }
            IndexNegation::Plain => self.pred.find_matching(storage, results),
        }
    }

    fn evaluate(&self, storage: &MapStorage<T>, id: StorageId) -> bool {
        match self.negation {
            IndexNegation::Negated => !self.pred.matches(storage, id),
            IndexNegation::Plain => self.pred.matches(storage, id),
        }
    }
}

enum PredicateKind<'a, T> {
    Index(IndexPredBox<'a, T>),
    And(Vec<PredicateKind<'a, T>>),
    Or(Vec<PredicateKind<'a, T>>),
    All,
    None,
}

impl<T> PredicateKind<'_, T> {
    fn negate(self) -> Self {
        // Perform DeMorganization on the predicate, pushing negation down to the
        // leaves.
        match self {
            PredicateKind::Index(pred) => PredicateKind::Index(pred.negate()),
            PredicateKind::And(preds) => {
                PredicateKind::Or(preds.into_iter().map(PredicateKind::negate).collect())
            }
            PredicateKind::Or(preds) => {
                PredicateKind::And(preds.into_iter().map(PredicateKind::negate).collect())
            }
            PredicateKind::All => PredicateKind::None,
            PredicateKind::None => PredicateKind::All,
        }
    }
}

/// Helper enum for optimizing AND/OR expressions
#[derive(Debug, Clone, Default)]
enum Clauses<T> {
    #[default]
    None,
    Singleton(T),
    Multiple(Vec<T>),
}

impl<T> Clauses<T> {
    fn append(self, value: T) -> Self {
        self.extend(std::iter::once(value))
    }

    fn extend(self, mut items: impl Iterator<Item = T>) -> Self {
        match self {
            Clauses::None => match items.next() {
                Some(first) => Clauses::Singleton(first).extend(items),
                None => Clauses::None,
            },
            Clauses::Singleton(v) => match items.next() {
                Some(second) => Clauses::Multiple(vec![v, second]).extend(items),
                None => Clauses::Singleton(v),
            },
            Clauses::Multiple(mut v) => {
                v.extend(items);
                Clauses::Multiple(v)
            }
        }
    }

    fn extract(self, default: impl FnOnce() -> T, combine: impl FnOnce(Vec<T>) -> T) -> T {
        match self {
            Clauses::None => default(),
            Clauses::Singleton(v) => v,
            Clauses::Multiple(v) => combine(v),
        }
    }
}

impl<'a, T> PredicateKind<'a, T> {
    pub(super) fn collect(&self, storage: &MapStorage<T>, results: &mut HashSet<StorageId>) {
        match &self {
            PredicateKind::Index(pred) => pred.find_matching(storage, results),
            PredicateKind::And(preds) => {
                // TODO: Impelement some kind of basic optimization logic,
                // using the size hints when available.
                let mut pred_iter = preds.iter();
                let Some(first_pred) = pred_iter.next() else {
                    panic!("Empty AND expression in reified predicate")
                };
                first_pred.collect(storage, results);
                for pred in pred_iter {
                    pred.filter(storage, results);
                }
            }
            PredicateKind::Or(preds) => {
                for pred in preds {
                    pred.collect(storage, results);
                }
            }
            PredicateKind::All => results.extend(storage.all_ids()),
            PredicateKind::None => results.clear(),
        }
    }

    fn evaluate(&self, storage: &MapStorage<T>, id: StorageId) -> bool {
        match &self {
            PredicateKind::Index(pred) => pred.evaluate(storage, id),
            PredicateKind::And(preds) => preds.iter().all(|pred| pred.evaluate(storage, id)),
            PredicateKind::Or(preds) => preds.iter().any(|pred| pred.evaluate(storage, id)),
            PredicateKind::All => true,
            PredicateKind::None => false,
        }
    }

    fn filter(&self, storage: &MapStorage<T>, ids: &mut HashSet<StorageId>) {
        ids.retain(|id| self.evaluate(storage, *id));
    }

    fn new_and(pred_iter: impl Iterator<Item = PredicateKind<'a, T>>) -> Self {
        // We effectively do a fold, but we have to be able to short-circuit
        // if we see a PredicateKind::None.
        let mut clauses = Clauses::None;
        for pred in pred_iter {
            match pred {
                PredicateKind::None => return PredicateKind::None,
                PredicateKind::All => {}
                PredicateKind::And(preds) => clauses = clauses.extend(preds.into_iter()),
                pred => clauses = clauses.append(pred),
            }
        }
        clauses.extract(|| PredicateKind::All, |preds| PredicateKind::And(preds))
    }

    fn new_or(pred_iter: impl Iterator<Item = PredicateKind<'a, T>>) -> Self {
        let mut clauses = Clauses::None;
        for pred in pred_iter {
            match pred {
                PredicateKind::All => return PredicateKind::All,
                PredicateKind::None => {}
                PredicateKind::Or(preds) => clauses = clauses.extend(preds.into_iter()),
                pred => clauses = clauses.append(pred),
            }
        }
        clauses.extract(|| PredicateKind::None, |preds| PredicateKind::Or(preds))
    }

    fn new_not(pred: PredicateKind<'a, T>) -> Self {
        pred.negate()
    }

    fn new_index(pred: impl IndexPredicate<'a, T> + 'a) -> Self {
        PredicateKind::Index(IndexPredBox {
            pred: Box::new(pred),
            negation: IndexNegation::Plain,
        })
    }
}

pub(crate) struct Predicate<'a, T> {
    pred: PredicateKind<'a, T>,
}

impl<'a, T> Predicate<'a, T> {
    pub(crate) fn all() -> Self {
        Self {
            pred: PredicateKind::All,
        }
    }

    pub(crate) fn none() -> Self {
        Self {
            pred: PredicateKind::None,
        }
    }

    pub(crate) fn and(preds: impl Iterator<Item = Predicate<'a, T>>) -> Self {
        Self {
            pred: PredicateKind::new_and(preds.map(|pred| pred.pred)),
        }
    }

    pub(crate) fn or(preds: impl Iterator<Item = Predicate<'a, T>>) -> Self {
        Self {
            pred: PredicateKind::new_or(preds.map(|pred| pred.pred)),
        }
    }

    pub(crate) fn not(pred: Predicate<'a, T>) -> Self {
        Self {
            pred: PredicateKind::new_not(pred.pred),
        }
    }

    pub(super) fn index(pred: impl IndexPredicate<'a, T> + 'a) -> Self {
        Self {
            pred: PredicateKind::new_index(pred),
        }
    }

    pub(super) fn collect(&self, storage: &MapStorage<T>, results: &mut HashSet<StorageId>) {
        self.pred.collect(storage, results);
    }
}
