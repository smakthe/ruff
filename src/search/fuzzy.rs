//! Fuzzy search functionality

use serde::{Deserialize, Serialize};

/// Fuzzy matcher for approximate string matching
pub struct FuzzyMatcher {}

/// Fuzzy search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzySearchResult {
    pub text: String,
    pub score: f32,
    pub indices: Vec<usize>,
}

impl FuzzyMatcher {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}
