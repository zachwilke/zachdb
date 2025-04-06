use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a document in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Unique identifier for the document
    pub id: String,
    /// Document data as a JSON string
    #[serde(serialize_with = "serialize_json", deserialize_with = "deserialize_json")]
    pub data: serde_json::Value,
    /// Creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
}

// Serialize JSON value to string for bincode compatibility
fn serialize_json<S>(value: &serde_json::Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let json_string = value.to_string();
    serializer.serialize_str(&json_string)
}

// Deserialize JSON string back to Value
fn deserialize_json<'de, D>(deserializer: D) -> Result<serde_json::Value, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let json_string = String::deserialize(deserializer)?;
    serde_json::from_str(&json_string).map_err(serde::de::Error::custom)
}

impl Document {
    /// Creates a new document from the provided data
    pub fn new(data: serde_json::Value) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            id: Uuid::new_v4().to_string(),
            data,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Creates a document with a specific ID
    pub fn with_id(id: String, data: serde_json::Value) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            id,
            data,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Updates the document data
    pub fn update(&mut self, data: serde_json::Value) {
        self.data = data;
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

/// Represents a query operator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryOperator {
    /// Equality comparison
    Eq,
    /// Not equal
    Ne,
    /// Greater than
    Gt,
    /// Greater than or equal
    Gte,
    /// Less than
    Lt,
    /// Less than or equal
    Lte,
    /// Contains substring (for strings)
    Contains,
    /// Exists check (field exists)
    Exists,
    /// In array of values
    In,
}

/// Represents a query condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCondition {
    /// Field path (can be nested with dot notation)
    pub field: String,
    /// Operator for comparison
    pub operator: QueryOperator,
    /// Value to compare against
    pub value: serde_json::Value,
}

/// Represents a query to filter documents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    /// List of conditions (implicitly AND-ed together)
    pub conditions: Vec<QueryCondition>,
    /// Maximum number of results to return
    pub limit: Option<usize>,
    /// Number of results to skip
    pub skip: Option<usize>,
    /// Field to sort by and direction (true for ascending)
    pub sort_by: Option<(String, bool)>,
}

/// Database operation result type
pub type DbResult<T> = Result<T, DbError>;

/// Database error types
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// Error when collection not found
    #[error("Collection not found: {0}")]
    CollectionNotFound(String),
    
    /// Error when document not found
    #[error("Document not found: {0}")]
    DocumentNotFound(String),
    
    /// Error when collection already exists
    #[error("Collection already exists: {0}")]
    CollectionExists(String),
    
    /// Error when storage operation fails
    #[error("Storage error: {0}")]
    Storage(String),
    
    /// Error when IO operation fails
    #[error("IO error: {0}")]
    Io(String),
    
    /// Error with invalid data
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Error with invalid query
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
    
    /// Error with indexing
    #[error("Indexing error: {0}")]
    IndexError(String),
} 