//! Trait-based abstraction for TACT key storage
//!
//! This module defines a common interface for different TACT key storage backends,
//! allowing for pluggable storage implementations while maintaining API compatibility.

use std::collections::HashSet;

use crate::error::CryptoError;
use crate::keys::{TactKey, TactKeyStore};

/// Trait for TACT key storage backends
///
/// This trait defines the common interface that all key storage implementations
/// must provide. It supports both owned data (for keyring/database backends) and
/// borrowed data (for in-memory backends).
pub trait TactKeyProvider {
    /// Get a key by ID, returning owned data
    ///
    /// This method returns owned key data, which is suitable for all storage
    /// backends including keyring, database, and in-memory implementations.
    fn get_key(&self, id: u64) -> Result<Option<[u8; 16]>, CryptoError>;

    /// Add a key to the store
    fn add_key(&mut self, key: TactKey) -> Result<(), CryptoError>;

    /// Remove a key from the store
    fn remove_key(&mut self, id: u64) -> Result<Option<[u8; 16]>, CryptoError>;

    /// Get the number of keys in the store
    ///
    /// Note: For some backends (like keyring), this may be an approximation
    /// or may require scanning all entries.
    fn key_count(&self) -> Result<usize, CryptoError>;

    /// Check if the store appears empty
    fn is_empty(&self) -> Result<bool, CryptoError> {
        Ok(self.key_count()? == 0)
    }

    /// Check if a key exists in the store
    fn contains_key(&self, id: u64) -> Result<bool, CryptoError> {
        Ok(self.get_key(id)?.is_some())
    }

    /// Get all key IDs in the store
    ///
    /// Note: For some backends (like keyring), this may not be efficiently
    /// supported and may return an empty vector or require separate indexing.
    fn list_key_ids(&self) -> Result<Vec<u64>, CryptoError>;

    /// Load keys from an external source
    ///
    /// The default implementation does nothing. Specific implementations
    /// can override this to support loading from files, databases, etc.
    fn load_keys(&mut self) -> Result<usize, CryptoError> {
        Ok(0)
    }

    /// Save keys to persistent storage
    ///
    /// The default implementation does nothing. Specific implementations
    /// can override this to support persistence.
    fn save_keys(&self) -> Result<(), CryptoError> {
        Ok(())
    }
}

/// Extension trait for storage backends that support iteration
pub trait TactKeyIterator: TactKeyProvider {
    /// Iterator type for keys
    type KeyIter: Iterator<Item = Result<TactKey, CryptoError>>;

    /// Iterate over all keys in the store
    ///
    /// Note: This may be expensive for some backends and should be used judiciously.
    fn iter_keys(&self) -> Result<Self::KeyIter, CryptoError>;
}

/// Configuration trait for storage backends
pub trait TactKeyStoreConfig {
    /// Configuration type for this storage backend
    type Config;

    /// Create a new instance with the given configuration
    fn with_config(config: Self::Config) -> Result<Self, CryptoError>
    where
        Self: Sized;

    /// Get the current configuration
    fn config(&self) -> &Self::Config;

    /// Update the configuration
    ///
    /// Returns true if the configuration was successfully updated,
    /// false if the update would require recreating the store.
    fn update_config(&mut self, config: Self::Config) -> Result<bool, CryptoError>;
}

/// Unified key store that can use any backend
#[derive(Debug)]
pub struct UnifiedKeyStore<T: TactKeyProvider> {
    backend: T,
}

impl<T: TactKeyProvider> UnifiedKeyStore<T> {
    /// Create a new unified key store with the specified backend
    pub fn new(backend: T) -> Self {
        Self { backend }
    }

    /// Get the underlying backend
    pub fn backend(&self) -> &T {
        &self.backend
    }

    /// Get mutable access to the underlying backend
    pub fn backend_mut(&mut self) -> &mut T {
        &mut self.backend
    }

    /// Consume the unified store and return the backend
    pub fn into_backend(self) -> T {
        self.backend
    }
}

impl<T: TactKeyProvider> TactKeyProvider for UnifiedKeyStore<T> {
    fn get_key(&self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        self.backend.get_key(id)
    }

    fn add_key(&mut self, key: TactKey) -> Result<(), CryptoError> {
        self.backend.add_key(key)
    }

    fn remove_key(&mut self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        self.backend.remove_key(id)
    }

    fn key_count(&self) -> Result<usize, CryptoError> {
        self.backend.key_count()
    }

    fn list_key_ids(&self) -> Result<Vec<u64>, CryptoError> {
        self.backend.list_key_ids()
    }

    fn load_keys(&mut self) -> Result<usize, CryptoError> {
        self.backend.load_keys()
    }

    fn save_keys(&self) -> Result<(), CryptoError> {
        self.backend.save_keys()
    }
}

/// Implementation of `TactKeyProvider` for the standard in-memory `TactKeyStore`
impl TactKeyProvider for TactKeyStore {
    fn get_key(&self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        Ok(self.get(id).copied())
    }

    fn add_key(&mut self, key: TactKey) -> Result<(), CryptoError> {
        self.add(key);
        Ok(())
    }

    fn remove_key(&mut self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        Ok(self.remove(id))
    }

    fn key_count(&self) -> Result<usize, CryptoError> {
        Ok(self.len())
    }

    fn list_key_ids(&self) -> Result<Vec<u64>, CryptoError> {
        Ok(self.iter().map(|k| k.id).collect())
    }

    fn load_keys(&mut self) -> Result<usize, CryptoError> {
        let _initial_count = self.len();
        // Load hardcoded keys - this is done automatically in TactKeyStore::new()
        // For an existing store, we would need to track what's already loaded
        // For now, return 0 as hardcoded keys are loaded at creation
        Ok(0)
    }
}

/// Chain-of-responsibility key provider.
///
/// Tries multiple [`TactKeyProvider`] backends in order, returning the first
/// hit. This matches Agent.exe's `KeyGetter::LookupKey` pattern where keys are
/// looked up across in-memory cache, Armadillo files, and keyring config.
///
/// Mutations (`add_key`, `remove_key`) operate on the primary (first) provider
/// only. An empty chain returns `Ok(None)` for lookups and errors for mutations.
pub struct ChainedKeyProvider {
    providers: Vec<Box<dyn TactKeyProvider>>,
}

impl ChainedKeyProvider {
    /// Create a chain with a single primary provider.
    pub fn new(primary: Box<dyn TactKeyProvider>) -> Self {
        Self {
            providers: vec![primary],
        }
    }

    /// Create an empty chain with no providers.
    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Append a fallback provider to the chain.
    ///
    /// Providers are tried in insertion order: the first added via [`new`](Self::new)
    /// is the primary, subsequent ones added via `push` are fallbacks.
    pub fn push(&mut self, provider: Box<dyn TactKeyProvider>) {
        self.providers.push(provider);
    }
}

impl TactKeyProvider for ChainedKeyProvider {
    fn get_key(&self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        for provider in &self.providers {
            if let Some(key) = provider.get_key(id)? {
                return Ok(Some(key));
            }
        }
        Ok(None)
    }

    fn add_key(&mut self, key: TactKey) -> Result<(), CryptoError> {
        match self.providers.first_mut() {
            Some(primary) => primary.add_key(key),
            None => Err(CryptoError::InvalidKeyFormat(
                "no providers in chain".to_string(),
            )),
        }
    }

    fn remove_key(&mut self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
        match self.providers.first_mut() {
            Some(primary) => primary.remove_key(id),
            None => Ok(None),
        }
    }

    fn key_count(&self) -> Result<usize, CryptoError> {
        let mut total = 0;
        for provider in &self.providers {
            total += provider.key_count()?;
        }
        Ok(total)
    }

    fn list_key_ids(&self) -> Result<Vec<u64>, CryptoError> {
        let mut seen = HashSet::new();
        for provider in &self.providers {
            for id in provider.list_key_ids()? {
                seen.insert(id);
            }
        }
        Ok(seen.into_iter().collect())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Simple in-memory implementation for testing
    struct TestKeyStore {
        keys: HashMap<u64, [u8; 16]>,
    }

    impl TestKeyStore {
        fn new() -> Self {
            Self {
                keys: HashMap::new(),
            }
        }
    }

    impl TactKeyProvider for TestKeyStore {
        fn get_key(&self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
            Ok(self.keys.get(&id).copied())
        }

        fn add_key(&mut self, key: TactKey) -> Result<(), CryptoError> {
            self.keys.insert(key.id, key.key);
            Ok(())
        }

        fn remove_key(&mut self, id: u64) -> Result<Option<[u8; 16]>, CryptoError> {
            Ok(self.keys.remove(&id))
        }

        fn key_count(&self) -> Result<usize, CryptoError> {
            Ok(self.keys.len())
        }

        fn list_key_ids(&self) -> Result<Vec<u64>, CryptoError> {
            Ok(self.keys.keys().copied().collect())
        }
    }

    #[test]
    fn test_unified_key_store() {
        let backend = TestKeyStore::new();
        let mut store = UnifiedKeyStore::new(backend);

        let test_key = TactKey::new(0x1234, [0x42; 16]);

        // Test add
        store
            .add_key(test_key)
            .expect("Adding test key should succeed");
        assert_eq!(
            store.key_count().expect("Getting key count should succeed"),
            1
        );

        // Test get
        let retrieved = store
            .get_key(0x1234)
            .expect("Getting test key should succeed");
        assert_eq!(retrieved, Some([0x42; 16]));

        // Test remove
        let removed = store
            .remove_key(0x1234)
            .expect("Removing test key should succeed");
        assert_eq!(removed, Some([0x42; 16]));
        assert_eq!(
            store
                .key_count()
                .expect("Getting key count after removal should succeed"),
            0
        );
    }

    #[test]
    fn test_contains_key() {
        let backend = TestKeyStore::new();
        let mut store = UnifiedKeyStore::new(backend);

        let test_key = TactKey::new(0x1234, [0x42; 16]);
        store
            .add_key(test_key)
            .expect("Adding test key should succeed");

        assert!(
            store
                .contains_key(0x1234)
                .expect("Checking if key exists should succeed")
        );
        assert!(
            !store
                .contains_key(0x5678)
                .expect("Checking if non-existent key exists should succeed")
        );
    }

    #[test]
    fn test_list_key_ids() {
        let backend = TestKeyStore::new();
        let mut store = UnifiedKeyStore::new(backend);

        let key1 = TactKey::new(0x1234, [0x42; 16]);
        let key2 = TactKey::new(0x5678, [0x43; 16]);

        store
            .add_key(key1)
            .expect("Adding first key should succeed");
        store
            .add_key(key2)
            .expect("Adding second key should succeed");

        let mut ids = store
            .list_key_ids()
            .expect("Listing key IDs should succeed");
        ids.sort_unstable();
        assert_eq!(ids, vec![0x1234, 0x5678]);
    }

    // --- ChainedKeyProvider tests ---

    #[allow(clippy::unnecessary_box_returns)]
    fn make_store_with(keys: &[(u64, [u8; 16])]) -> Box<TestKeyStore> {
        let mut store = TestKeyStore::new();
        for &(id, key) in keys {
            store.keys.insert(id, key);
        }
        Box::new(store)
    }

    #[test]
    fn chained_fallthrough_to_secondary() {
        let primary = make_store_with(&[]);
        let secondary = make_store_with(&[(0xAAAA, [0x11; 16])]);

        let mut chain = ChainedKeyProvider::new(primary);
        chain.push(secondary);

        let result = chain.get_key(0xAAAA).unwrap();
        assert_eq!(result, Some([0x11; 16]));
    }

    #[test]
    fn chained_primary_wins() {
        let primary = make_store_with(&[(0xAAAA, [0x11; 16])]);
        let secondary = make_store_with(&[(0xAAAA, [0x22; 16])]);

        let mut chain = ChainedKeyProvider::new(primary);
        chain.push(secondary);

        let result = chain.get_key(0xAAAA).unwrap();
        assert_eq!(result, Some([0x11; 16]));
    }

    #[test]
    fn chained_add_key_goes_to_primary() {
        let primary = make_store_with(&[]);
        let secondary = make_store_with(&[]);

        let mut chain = ChainedKeyProvider::new(primary);
        chain.push(secondary);

        chain.add_key(TactKey::new(0xBBBB, [0x33; 16])).unwrap();

        // Key is in primary (first provider)
        assert_eq!(
            chain.providers[0].get_key(0xBBBB).unwrap(),
            Some([0x33; 16])
        );
        // Key is not in secondary
        assert_eq!(chain.providers[1].get_key(0xBBBB).unwrap(), None);
    }

    #[test]
    fn chained_missing_from_all() {
        let primary = make_store_with(&[(0xAAAA, [0x11; 16])]);
        let secondary = make_store_with(&[(0xBBBB, [0x22; 16])]);

        let mut chain = ChainedKeyProvider::new(primary);
        chain.push(secondary);

        assert_eq!(chain.get_key(0xCCCC).unwrap(), None);
    }

    #[test]
    fn chained_empty_chain() {
        let chain = ChainedKeyProvider::empty();

        assert_eq!(chain.get_key(0xAAAA).unwrap(), None);
        assert_eq!(chain.key_count().unwrap(), 0);
        assert!(chain.list_key_ids().unwrap().is_empty());
    }

    #[test]
    fn chained_empty_add_key_errors() {
        let mut chain = ChainedKeyProvider::empty();
        let result = chain.add_key(TactKey::new(0xAAAA, [0x11; 16]));
        assert!(result.is_err());
    }

    #[test]
    fn chained_list_key_ids_deduplicates() {
        let primary = make_store_with(&[(0xAAAA, [0x11; 16]), (0xBBBB, [0x22; 16])]);
        let secondary = make_store_with(&[(0xBBBB, [0x33; 16]), (0xCCCC, [0x44; 16])]);

        let mut chain = ChainedKeyProvider::new(primary);
        chain.push(secondary);

        let mut ids = chain.list_key_ids().unwrap();
        ids.sort_unstable();
        assert_eq!(ids, vec![0xAAAA, 0xBBBB, 0xCCCC]);
    }
}
