//! Search functionality module
//!
//! This module provides comprehensive search capabilities including:
//! - Full-text search across sessions and messages
//! - Search indexing and query processing
//! - Fuzzy matching and ranking
//! - Search result management
//! - Tantivy-based persistent search (production-ready)

pub mod background_indexing;
pub mod fuzzy;
pub mod index;
pub mod query;
pub mod ranking;
pub mod tantivy_backend;
pub mod tantivy_schema;
pub mod unified;

pub use background_indexing::{BackgroundIndexer, IndexingConfig, IndexingStats};
pub use fuzzy::{FuzzyMatcher, FuzzySearchResult};
pub use index::{IndexManager, SearchIndex};
pub use query::{SearchFilters, SearchQuery, SearchResult};
pub use ranking::{RankingAlgorithm, SearchRanker};
pub use tantivy_backend::{IndexStatistics, TantivyMessageSearchIndex};
pub use tantivy_schema::MessageIndexSchema;
pub use unified::{
    DateRange, MessageSearchResult, SearchBackend, SearchFilters as UnifiedSearchFilters,
    SearchResult as UnifiedSearchResult, SearchScope, SessionSearchResult, UnifiedSearchQuery,
};
