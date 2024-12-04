//! Types for defining symbols.
//!
//! Symbols serve two purposes: They are unique values that can be used to tie
//! together different parts of the code, and they are used during debugging to
//! provide the context they were made in.

use std::{
    borrow::Borrow,
    collections::hash_map,
    fmt::Debug,
    sync::{atomic::AtomicUsize, Arc},
};

// The main idea: Allocated memory is guaranteed to be unique, so we can use
// the memory address as a unique identifier. We have to be able to
// compare, hash, clone, and print these identifiers, and detect when
// they are dropped, so we use an Arc with our own use count to ensure that
// the pointer stays valid until the last clone is dropped.

// Because it's hard to manage ownership for key values, we need to manage a
// separate WeakSymbol type, that can be used as a key in a HashMap or BTreeMap,
// but still maintains the guarantees of the map type (i.e. still preserves the
// same identity). We are using the pointer value itself, but Weak does not
// guarantee that if there are no more strong references that as_ptr() will
// return the same value, so we need to use separate tracking to manage it.
//
// Right now, we're using a technique that requries no unsafe code, but this
// could be made more efficient, as we have to store the pointer twice.

struct UniquePayload {
    /// Keeps a count of the number of [`Symbol`]s that are still alive.
    ///
    /// Note that this does not carry any safety semantics, so ordering for
    /// this value is not important.
    strong_count: AtomicUsize,
    name: Option<String>,
}

pub struct SymbolId(Arc<UniquePayload>);

/// All of the methods on SymbolId are private, as we're hiding the fact that
/// the ID is the internal payload of an Arc.
impl SymbolId {
    fn new() -> Self {
        Self(Arc::new(UniquePayload {
            strong_count: AtomicUsize::new(0),
            name: None,
        }))
    }

    fn with_name(name: String) -> Self {
        Self(Arc::new(UniquePayload {
            strong_count: AtomicUsize::new(0),
            name: Some(name),
        }))
    }

    fn inc_strong_count(&self) {
        self.0
            .strong_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn dec_strong_count(&self) {
        self.0
            .strong_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn as_ptr(&self) -> *const UniquePayload {
        Arc::as_ptr(&self.0)
    }

    fn strong_count(&self) -> usize {
        self.0
            .strong_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// This is a private method so that we can expose SymbolId as a borrowable
    /// value without letting clients clone it.
    fn do_clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }

    fn fmt_dbg(&self, prefix: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0.name {
            Some(name) => write!(f, "[{}#{:?}: {}]", prefix, self.as_ptr(), name),
            None => write!(f, "[{}#{:?}]", prefix, self.as_ptr()),
        }
    }
}

impl Debug for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_dbg("ID: ", f)
    }
}

impl std::cmp::PartialEq for SymbolId {
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr().eq(&other.as_ptr())
    }
}

impl std::cmp::Eq for SymbolId {}

impl std::cmp::PartialOrd for SymbolId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for SymbolId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ptr().cmp(&other.as_ptr())
    }
}

impl std::hash::Hash for SymbolId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state);
    }
}

/// A unique symbol that can be used to identify a value.
///
/// This symbol is guaranteed to be unique from all other symbols generated
/// with new() or with_name(). It can be cloned, compared, hashed, and printed.
///
/// It can also be detected when it is the only clone left, to ensure that any
/// values using it as a key are unreachable.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol(SymbolId);

impl Symbol {
    fn new_from_id(contents: SymbolId) -> Self {
        contents.inc_strong_count();
        Self(contents)
    }
    /// Creates a new unique symbol.
    ///
    /// It is guaranteed to be unique by comparison and hash with all other
    /// symbols at the time it's created.
    pub fn new() -> Self {
        Self::new_from_id(SymbolId::new())
    }

    /// Creates a new unique symbol with a name.
    ///
    /// This is identical to new() above, but provides the created symbol with
    /// a name for debugging purposes.
    pub fn with_name(name: impl Into<String>) -> Self {
        Self::new_from_id(SymbolId::with_name(name.into()))
    }

    pub fn id(&self) -> &SymbolId {
        &self.0
    }

    fn downgrade(&self) -> WeakSymbol {
        WeakSymbol::weak_from_id(self.0.do_clone())
    }
}

impl Clone for Symbol {
    fn clone(&self) -> Self {
        Self::new_from_id(self.0.do_clone())
    }
}

impl Drop for Symbol {
    fn drop(&mut self) {
        self.0.dec_strong_count();
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id().fmt_dbg("", f)
    }
}

impl Borrow<SymbolId> for Symbol {
    fn borrow(&self) -> &SymbolId {
        self.id()
    }
}

/// A weak symbol that can be used as a key in a map.
///
/// This is a symbol that is guaranteed to be unique, and is associated with
/// zero or more live instances of a [`Symbol`]. It can be used as a key in a
/// map.
///
/// This can be used to handle weak keys in a map, where we want to detect when
/// it is impossible to ever be given a symbol that maps to this key.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct WeakSymbol(SymbolId);

impl WeakSymbol {
    fn weak_from_id(id: SymbolId) -> Self {
        Self(id)
    }

    /// Returns true iff there are no strong symbol objects left for this object.
    pub fn strong_syms_exist(&self) -> bool {
        self.0.strong_count() > 0
    }

    /// Returns the ID of the weak symbol.
    pub fn id(&self) -> &SymbolId {
        &self.0
    }

    pub fn try_upgrade(&self) -> Option<Symbol> {
        if self.strong_syms_exist() {
            Some(Symbol::new_from_id(self.0.do_clone()))
        } else {
            None
        }
    }
}

impl Clone for WeakSymbol {
    fn clone(&self) -> Self {
        Self::weak_from_id(self.0.do_clone())
    }
}

impl Debug for WeakSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id().fmt_dbg("Weak: ", f)
    }
}

impl Borrow<SymbolId> for WeakSymbol {
    fn borrow(&self) -> &SymbolId {
        self.id()
    }
}

#[derive(Clone)]
pub struct WeakSymbolMap<V> {
    map: std::collections::HashMap<WeakSymbol, V>,
}

impl<V> WeakSymbolMap<V> {
    pub fn new() -> Self {
        Self {
            map: std::collections::HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: &Symbol, value: V) -> Option<V> {
        self.map.insert(key.downgrade(), value)
    }

    pub fn try_insert_mut(&mut self, key: &Symbol, value: V) -> Result<&mut V, &V> {
        match self.map.entry(key.downgrade()) {
            hash_map::Entry::Occupied(occ) => Err(occ.into_mut()),
            hash_map::Entry::Vacant(vac) => Ok(vac.insert(value)),
        }
    }

    pub fn insert_if_empty<F: FnOnce() -> V>(&mut self, key: &Symbol, value_fn: F) -> Option<&V> {
        match self.map.entry(key.downgrade()) {
            hash_map::Entry::Occupied(occ) => Some(occ.into_mut()),
            hash_map::Entry::Vacant(vac) => {
                vac.insert(value_fn());
                None
            }
        }
    }

    pub fn get(&self, key: &Symbol) -> Option<&V> {
        self.map.get(key.id())
    }

    pub fn remove(&mut self, key: &Symbol) -> Option<V> {
        self.map.remove(key.id())
    }

    pub fn contains_key(&self, key: &Symbol) -> bool {
        self.map.contains_key(key.id())
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn clear(&mut self) {
        self.map.clear()
    }

    pub fn keys(&self) -> impl Iterator<Item = Symbol> + '_ {
        self.map.keys().filter_map(|k| k.try_upgrade())
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.map
            .iter()
            .filter_map(|(k, v)| k.try_upgrade().map(|_| v))
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.map
            .iter_mut()
            .filter_map(|(k, v)| k.try_upgrade().map(|_| v))
    }

    /// Cleans up the map by removing any entries that have no strong symbols.
    pub fn clean(&mut self) {
        self.map.retain(|k, _| k.strong_syms_exist());
    }
}

impl<V: Debug> Debug for WeakSymbolMap<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeakSymbolMap")
            .field(
                "entries",
                &self
                    .map
                    .iter()
                    .filter_map(|(k, v)| k.try_upgrade().map(|_| v))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

pub struct WeakSymbolMapIntoIter<V> {
    inner: hash_map::IntoIter<WeakSymbol, V>,
}

impl<V> Iterator for WeakSymbolMapIntoIter<V> {
    type Item = (Symbol, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((k, v)) => {
                    if let Some(k) = k.try_upgrade() {
                        return Some((k, v));
                    }
                }
                None => return None,
            }
        }
    }
}

pub struct WeakSymbolMapIter<'a, V> {
    inner: hash_map::Iter<'a, WeakSymbol, V>,
}

impl<'a, V> Iterator for WeakSymbolMapIter<'a, V> {
    type Item = (Symbol, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((k, v)) => {
                    if let Some(k) = k.try_upgrade() {
                        return Some((k, v));
                    }
                }
                None => return None,
            }
        }
    }
}

impl<V> IntoIterator for WeakSymbolMap<V> {
    type Item = (Symbol, V);

    type IntoIter = WeakSymbolMapIntoIter<V>;

    fn into_iter(self) -> Self::IntoIter {
        WeakSymbolMapIntoIter {
            inner: self.map.into_iter(),
        }
    }
}

impl<'a, V> IntoIterator for &'a WeakSymbolMap<V> {
    type Item = (Symbol, &'a V);

    type IntoIter = WeakSymbolMapIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        WeakSymbolMapIter {
            inner: self.map.iter(),
        }
    }
}

impl<V> Default for WeakSymbolMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::*;

    #[test]
    fn test_new_symbols_are_unique() {
        let sym1 = Symbol::new();
        let sym2 = Symbol::new();
        assert_ne!(sym1, sym2);
    }

    #[test]
    fn test_named_symbols_are_unique() {
        let sym1 = Symbol::with_name("test1");
        let sym2 = Symbol::with_name("test1"); // Same name
        assert_ne!(sym1, sym2);
    }

    #[test]
    fn test_cloned_symbols_are_equal() {
        let sym1 = Symbol::new();
        let sym2 = sym1.clone();
        assert_eq!(sym1, sym2);
    }

    #[test]
    fn test_symbol_hash_consistency() {
        let mut map = HashMap::new();
        let sym1 = Symbol::new();
        let sym2 = sym1.clone();

        map.insert(sym1, "value");
        assert_eq!(map.get(&sym2), Some(&"value"));
    }

    #[test]
    fn test_symbol_debug_format() {
        let sym = Symbol::with_name("test_symbol");
        let debug_str = format!("{:?}", sym);
        assert!(debug_str.contains("test_symbol"));
        assert!(
            debug_str.starts_with("[#"),
            "Expected \"[#\", got {:?}",
            debug_str
        );
        assert!(
            debug_str.ends_with("]"),
            "Expected \"], got {:?}",
            debug_str
        );
    }

    #[test]
    fn test_symbol_default() {
        let sym1 = Symbol::default();
        let sym2 = Symbol::default();
        assert_ne!(sym1, sym2); // Default should create unique symbols
    }

    #[test]
    fn test_symbol_ordering_consistency() {
        let sym1 = Symbol::new();
        let sym2 = Symbol::new();
        let sym1_clone = sym1.clone();

        // Test reflexivity
        assert_eq!(sym1.cmp(&sym1), std::cmp::Ordering::Equal);

        // Test clone ordering
        assert_eq!(sym1.cmp(&sym1_clone), std::cmp::Ordering::Equal);

        // Test consistency
        let first_cmp = sym1.cmp(&sym2);
        assert_eq!(sym1.cmp(&sym2), first_cmp); // Should be consistent
    }

    #[test]
    fn test_symbol_in_collections() {
        let mut set = HashSet::new();
        let sym1 = Symbol::new();
        let sym1_clone = sym1.clone();
        let sym2 = Symbol::new();

        set.insert(sym1);
        assert!(set.contains(&sym1_clone));
        assert!(!set.contains(&sym2));
    }

    #[test]
    fn test_weak_symbol_creation() {
        let sym = Symbol::new();
        let weak_sym = sym.downgrade();
        assert_eq!(sym.id(), weak_sym.id());
    }

    #[test]
    fn test_weak_symbol_debug_format() {
        let sym = Symbol::with_name("test_symbol");
        let weak_sym = sym.downgrade();
        let debug_str = format!("{:?}", weak_sym);
        assert!(debug_str.contains("test_symbol"));
        assert!(debug_str.starts_with("[Weak: #"));
        assert!(debug_str.ends_with("]"));
    }

    #[test]
    fn test_weak_symbol_ordering_consistency() {
        let sym1 = Symbol::new();
        let sym2 = Symbol::new();
        let weak_sym1 = sym1.downgrade();
        let weak_sym2 = sym2.downgrade();

        // Test reflexivity
        assert_eq!(weak_sym1.cmp(&weak_sym1), std::cmp::Ordering::Equal);

        // Test consistency
        let first_cmp = weak_sym1.cmp(&weak_sym2);
        assert_eq!(weak_sym1.cmp(&weak_sym2), first_cmp); // Should be consistent
    }

    #[test]
    fn test_weak_symbol_in_collections() {
        let mut set = HashSet::new();
        let sym1 = Symbol::new();
        let weak_sym1 = sym1.downgrade();
        let weak_sym1_clone = weak_sym1.clone();
        let sym2 = Symbol::new();
        let weak_sym2 = sym2.downgrade();

        set.insert(weak_sym1);
        assert!(set.contains(&weak_sym1_clone));
        assert!(!set.contains(&weak_sym2));
    }
}
