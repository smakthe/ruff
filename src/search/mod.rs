//! Search functionality module
//! 
//! This module provides comprehensive search capabilities including:
//! - Full-text search across sessions and messages
//! - Search indexing and query processing
//! - Fuzzy matching and ranking
//! - Search result management

pub mod index;
pub mod query;
pub mod ranking;
pub mod fuzzy;
pub mod background_indexing;

pub use index::{SearchIndex, IndexManager};
pub use query::{SearchQuery, SearchFilters, SearchResult};
pub use ranking::{RankingAlgorithm, SearchRanker};
pub use fuzzy::{FuzzyMatcher, FuzzySearchResult};
pub use background_indexing::{BackgroundIndexer, IndexingConfig, IndexingStats};