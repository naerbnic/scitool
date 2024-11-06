//! Types for defining symbols.
//!
//! Symbols serve two purposes: They are unique values that can be used to tie
//! together different parts of the code, and they are used during debugging to
//! provide the context they were made in.

use std::sync::Arc;

// The main idea: Allocated memory is guaranteed to be unique, so we can use
// the memory address as a unique identifier. We have to be able to
// compare, hash, clone, and print these identifiers, and detect when
// they are dropped, so we use an Arc with our own use count to ensure that
// the pointer stays valid until the last clone is dropped.

struct UniquePayload {
    name: Option<String>,
}

/// A unique symbol that can be used to identify a value.
///
/// This symbol is guaranteed to be unique from all other symbols generated
/// with new() or with_name(). It can be cloned, compared, hashed, and printed.
///
/// It can also be detected when it is the only clone left, to ensure that any
/// values using it as a key are unreachable.
#[derive(Clone)]
pub struct Symbol(Arc<UniquePayload>);

impl Symbol {
    /// Creates a new unique symbol.
    ///
    /// It is guaranteed to be unique by comparison and hash with all other
    /// symbols at the time it's created.
    pub fn new() -> Self {
        Self(Arc::new(UniquePayload { name: None }))
    }

    /// Creates a new unique symbol with a name.
    ///
    /// This is identical to new() above, but provides the created symbol with
    /// a name for debugging purposes.
    pub fn with_name(name: impl Into<String>) -> Self {
        Self(Arc::new(UniquePayload {
            name: Some(name.into()),
        }))
    }

    fn ptr(&self) -> *const UniquePayload {
        Arc::as_ptr(&self.0)
    }

    /// Returns true iff this is the only clone of this symbol.
    ///
    /// This operates on a mutable value to ensure the symbol is not shared
    /// with other threads through the same reference.
    pub fn is_unique(&mut self) -> bool {
        Arc::get_mut(&mut self.0).is_some()
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0.name {
            Some(name) => write!(f, "[#{:0X?}: {}]", self.ptr(), name),
            None => write!(f, "[#{:0X?}]", self.ptr()),
        }
    }
}

impl std::cmp::PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::cmp::Eq for Symbol {}

impl std::cmp::PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for Symbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ptr().cmp(&other.ptr())
    }
}

impl std::hash::Hash for Symbol {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr().hash(state);
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
        assert!(debug_str.starts_with("[#"));
        assert!(debug_str.ends_with("]"));
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
}
