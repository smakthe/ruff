//! Search indexing functionality

/// Search index for full-text search
pub struct SearchIndex {}

/// Index manager
pub struct IndexManager {}

impl SearchIndex {
    pub fn new() -> Self {
        Self {}
    }
}

impl IndexManager {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for IndexManager {
    fn default() -> Self {
        Self::new()
    }
}