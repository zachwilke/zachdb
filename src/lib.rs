pub mod core;
pub mod storage;
pub mod indexing;
pub mod api;
pub mod config;
pub mod utils;

// Re-export main types for easier access
pub use core::{Database, Document, Query};
pub use core::types::{QueryOperator, QueryCondition};
pub use api::{create_api_router, AppState}; 