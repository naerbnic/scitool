//! Expressions, which can be used to query the map.

use std::{collections::HashSet, fmt::Debug};

use crate::collections::indexed_map::index::ManagedIndex;

use super::{
    MapStorage, StorageId,
    index_table::{IndexId, IndexTable},
};

/// Helper type to allow access to the indexes and storage in a predicate.
pub(super) struct PredicateContext<'a, T> {
    indexes: &'a IndexTable<T>,
    storage: &'a MapStorage<T>,
}

impl<'a, T> PredicateContext<'a, T>
where
    T: 'static,
{
    pub(super) fn new(indexes: &'a IndexTable<T>, storage: &'a MapStorage<T>) -> Self {
        Self { indexes, storage }
    }

    pub(super) fn index_by_id<I>(&self, id: &IndexId<I>) -> &I
    where
        I: ManagedIndex<T>,
    {
        self.indexes.get(id).expect("Invalid id")
    }
}

/// The implementation of a predicate on a single index.
///
/// Indexes must implement this in order to be used in an expression.
pub(super) trait IndexPredicate<'a, T>: Debug {
    type Index: ManagedIndex<T>;
    /// Returns a size hint for the number of entries in the index that match the predicate.
    fn size_hint(
        &self,
        _index: &Self::Index,
        _storage: &MapStorage<T>,
        _negation: IndexNegation,
    ) -> (usize, Option<usize>) {
        (0, None)
    }

    /// Adds all ids of entries in the index that match the predicate to the
    /// results set.
    fn find_matching(
        &self,
        index: &Self::Index,
        storage: &MapStorage<T>,
        results: &mut HashSet<StorageId>,
    );

    /// If there is an optimization that allows us to find all ids of entries
    /// in the index that do not match the predicate, returns Ok(()). Otherwise,
    /// returns Err(()).
    fn try_find_non_matching(
        &self,
        _index: &Self::Index,
        _storage: &MapStorage<T>,
        _results: &mut HashSet<StorageId>,
    ) -> Result<(), ()> {
        Err(())
    }

    /// Returns whether a specific entry matches the predicate.
    fn matches(&self, index: &Self::Index, storage: &MapStorage<T>, id: StorageId) -> bool;
}

/// The implementation of a predicate on a single index.
///
/// Indexes must implement this in order to be used in an expression.
trait IndexPredicateObject<'a, T>: Debug {
    /// Returns a size hint for the number of entries in the index that match the predicate.
    fn size_hint(
        &self,
        context: &PredicateContext<T>,
        negation: IndexNegation,
    ) -> (usize, Option<usize>);

    /// Adds all ids of entries in the index that match the predicate to the
    /// results set.
    fn find_matching(&self, context: &PredicateContext<T>, results: &mut HashSet<StorageId>);

    /// If there is an optimization that allows us to find all ids of entries
    /// in the index that do not match the predicate, returns Ok(()). Otherwise,
    /// returns Err(()).
    fn try_find_non_matching(
        &self,
        context: &PredicateContext<T>,
        results: &mut HashSet<StorageId>,
    ) -> Result<(), ()>;

    /// Returns whether a specific entry matches the predicate.
    fn matches(&self, context: &PredicateContext<T>, id: StorageId) -> bool;
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

struct IndexPredImpl<'a, T, P>
where
    P: IndexPredicate<'a, T>,
{
    index_id: IndexId<P::Index>,
    pred: P,
}

impl<'a, T, P> Debug for IndexPredImpl<'a, T, P>
where
    P: IndexPredicate<'a, T> + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexPredImpl")
            .field("index_id", &self.index_id)
            .field("pred", &self.pred)
            .finish()
    }
}

impl<'a, T, P> IndexPredicateObject<'a, T> for IndexPredImpl<'a, T, P>
where
    T: 'static,
    P: IndexPredicate<'a, T>,
{
    fn size_hint(
        &self,
        context: &PredicateContext<T>,
        negation: IndexNegation,
    ) -> (usize, Option<usize>) {
        self.pred.size_hint(
            context.index_by_id(&self.index_id),
            context.storage,
            negation,
        )
    }

    fn find_matching(&self, context: &PredicateContext<T>, results: &mut HashSet<StorageId>) {
        self.pred.find_matching(
            context.index_by_id(&self.index_id),
            context.storage,
            results,
        );
    }

    fn try_find_non_matching(
        &self,
        context: &PredicateContext<T>,
        results: &mut HashSet<StorageId>,
    ) -> Result<(), ()> {
        self.pred.try_find_non_matching(
            context.index_by_id(&self.index_id),
            context.storage,
            results,
        )
    }

    fn matches(&self, context: &PredicateContext<T>, id: StorageId) -> bool {
        self.pred
            .matches(context.index_by_id(&self.index_id), context.storage, id)
    }
}

/// A struct that contains an index predicate, with optional negation.
struct IndexPredLeaf<'a, T> {
    pred: Box<dyn IndexPredicateObject<'a, T> + 'a>,
    negation: IndexNegation,
}

impl<T> IndexPredLeaf<'_, T> {
    fn negate(mut self) -> Self {
        self.negation = self.negation.negate();
        self
    }

    fn size_hint(&self, context: &PredicateContext<T>) -> (usize, Option<usize>) {
        self.pred.size_hint(context, self.negation)
    }

    fn find_matching(&self, context: &PredicateContext<T>, results: &mut HashSet<StorageId>) {
        match self.negation {
            IndexNegation::Negated => {
                if let Ok(()) = self.pred.try_find_non_matching(context, results) {
                    // There was an optimization in this implementation, so it should
                    // have already added all non-matching ids to the results set.
                    return;
                }

                // Go the slow path: Go through each id and add it to the results set
                // if it does not match the predicate.
                for id in context.storage.all_ids() {
                    if !self.pred.matches(context, id) {
                        results.insert(id);
                    }
                }
            }
            IndexNegation::Plain => self.pred.find_matching(context, results),
        }
    }

    fn evaluate(&self, context: &PredicateContext<T>, id: StorageId) -> bool {
        match self.negation {
            IndexNegation::Negated => !self.pred.matches(context, id),
            IndexNegation::Plain => self.pred.matches(context, id),
        }
    }
}

/// Represents the different kinds of reified predicates.
///
/// Notably, this does not include negation, as that is pushed to the
/// leaves of the expression tree.
enum PredicateKind<'a, T> {
    /// An index predicate. These are dynamic objects that evaluate predicates
    /// over a single index.
    Index(IndexPredLeaf<'a, T>),
    And(Vec<PredicateKind<'a, T>>),
    Or(Vec<PredicateKind<'a, T>>),
    All,
    None,
}

impl<'a, T> PredicateKind<'a, T> {
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

    fn size_hint(&self, context: &PredicateContext<T>) -> (usize, Option<usize>) {
        match self {
            PredicateKind::Index(pred) => pred.size_hint(context),
            PredicateKind::And(preds) => {
                // The upper bound of an And is the smallest upper bound of the
                // children.
                //
                // The lower bound is generally any number, as without further
                // information, any two children could be disjoint.
                let mut upper = None;
                for pred in preds {
                    let (_, child_upper) = pred.size_hint(context);
                    upper = match (upper, child_upper) {
                        (None, None) => None,
                        (Some(x), None) | (None, Some(x)) => Some(x),
                        (Some(x), Some(y)) => Some(std::cmp::min(x, y)),
                    };
                }
                (0, upper)
            }
            PredicateKind::Or(preds) => {
                let mut lower = 0;
                for pred in preds {
                    let (child_lower, _) = pred.size_hint(context);
                    lower = std::cmp::max(lower, child_lower);
                }
                (lower, None)
            }
            PredicateKind::All => (context.storage.size(), Some(context.storage.size())),
            PredicateKind::None => (0, Some(0)),
        }
    }

    fn collect(&self, context: &PredicateContext<T>, results: &mut HashSet<StorageId>) {
        match &self {
            PredicateKind::Index(pred) => pred.find_matching(context, results),
            PredicateKind::And(preds) => {
                // TODO: Impelement some kind of basic optimization logic,
                // using the size hints when available.
                let mut pred_iter = preds.iter();
                let Some(first_pred) = pred_iter.next() else {
                    panic!("Empty AND expression in reified predicate")
                };
                first_pred.collect(context, results);
                for pred in pred_iter {
                    pred.filter(context, results);
                }
            }
            PredicateKind::Or(preds) => {
                for pred in preds {
                    pred.collect(context, results);
                }
            }
            PredicateKind::All => results.extend(context.storage.all_ids()),
            PredicateKind::None => results.clear(),
        }
    }

    fn evaluate(&self, context: &PredicateContext<T>, id: StorageId) -> bool {
        match &self {
            PredicateKind::Index(pred) => pred.evaluate(context, id),
            PredicateKind::And(preds) => preds.iter().all(|pred| pred.evaluate(context, id)),
            PredicateKind::Or(preds) => preds.iter().any(|pred| pred.evaluate(context, id)),
            PredicateKind::All => true,
            PredicateKind::None => false,
        }
    }

    fn filter(&self, context: &PredicateContext<T>, ids: &mut HashSet<StorageId>) {
        ids.retain(|id| self.evaluate(context, *id));
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

    fn new_index<P>(index_id: IndexId<P::Index>, pred: P) -> Self
    where
        P: IndexPredicate<'a, T> + 'a,
        T: 'static,
    {
        PredicateKind::Index(IndexPredLeaf {
            pred: Box::new(IndexPredImpl { index_id, pred }),
            negation: IndexNegation::Plain,
        })
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

    /// Creates an index predicate that will use the contents of a single index
    /// object to evaluate the predicate.
    pub(super) fn index<P>(index_id: IndexId<P::Index>, pred: P) -> Self
    where
        P: IndexPredicate<'a, T> + 'a,
        T: 'static,
    {
        Self {
            pred: PredicateKind::new_index(index_id, pred),
        }
    }

    /// Collects all the [`StorageId`]s that match the predicate into
    /// `results`. Values are added to `results`.
    pub(super) fn collect(&self, context: &PredicateContext<T>, results: &mut HashSet<StorageId>) {
        self.pred.collect(context, results);
    }
}
