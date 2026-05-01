//! Performance optimization and monitoring module
//!
//! This module provides performance optimization tools, benchmarking,
//! and monitoring capabilities for the Ruff application.

pub mod benchmarks;

pub use benchmarks::{BenchmarkConfig, BenchmarkResult, MemoryUsage, PerformanceBenchmarks};
