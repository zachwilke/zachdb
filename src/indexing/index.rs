use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::Result;
use dashmap::DashMap;
use tantivy::{
    schema::{Field, Schema, INDEXED, STORED, TEXT, JsonObjectOptions, Type},
    Index as TantivyIndex, IndexWriter, Document as TantivyDoc, ReloadPolicy,
    collector::TopDocs, query::QueryParser, Term,
    directory::MmapDirectory,
    doc,
    DocAddress,
};

use serde_json::{Value, Map};
use crate::core::types::{Document, Query, QueryOperator, DbError};

/// Index error type
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Failed to create index: {0}")]
    CreateError(String),
    
    #[error("Failed to index document: {0}")]
    IndexingError(String),
    
    #[error("Failed to search: {0}")]
    SearchError(String),
    
    #[error("Failed to add field to index: {0}")]
    FieldError(String),
    
    #[error("Failed to open index directory: {0}")]
    IoError(String),
    
    #[error("Failed to create tantivy index: {0}")]
    TantivyError(String),
}

/// Index interface
pub trait Index: Send + Sync {
    /// Add a document to the index
    fn add_document(&self, document: &Document) -> Result<(), IndexError>;
    
    /// Update a document in the index
    fn update_document(&self, old_doc: &Document, new_doc: &Document) -> Result<(), IndexError>;
    
    /// Remove a document from the index
    fn remove_document(&self, document: &Document) -> Result<(), IndexError>;
    
    /// Search the index for documents matching the query
    fn search(&self, query: &Query) -> Result<Vec<String>, IndexError>;
    
    /// Commit any pending changes to the index
    fn commit(&self) -> Result<(), IndexError>;
}

/// Field index for a specific field
pub struct FieldIndex {
    /// The field's name
    field_name: String,
    /// Tantivy index for full-text search
    tantivy_index: TantivyIndex,
    /// Tantivy field for this field
    field: Field,
    /// ID field for document lookup
    id_field: Field,
    /// Index writer for adding/updating documents
    writer: Arc<RwLock<IndexWriter>>,
    /// In-memory hash index for exact lookups
    hash_index: DashMap<String, HashSet<String>>,
    /// Schema for the tantivy index
    schema: Schema,
}

impl FieldIndex {
    /// Create a schema for the index
    fn create_schema() -> (Schema, Field, Field) {
        let mut schema_builder = Schema::builder();
        
        // Add id field (stored and indexed)
        let id_field = schema_builder.add_text_field("id", STORED);
        
        // Add data field (stored and indexed for searching)
        let data_options = JsonObjectOptions::default()
            .set_stored()
            .set_indexing_options(tantivy::schema::TextFieldIndexing::default());
            
        let data_field = schema_builder.add_json_field("data", data_options);
        
        (schema_builder.build(), id_field, data_field)
    }
    
    /// Create a new field index
    pub fn new(name: &str, index_dir: Option<PathBuf>) -> Result<Self, IndexError> {
        let (schema, id_field, data_field) = Self::create_schema();
        
        // Create or open the index
        let index = match index_dir {
            Some(dir) => {
                // Create directory if it doesn't exist
                if !dir.exists() {
                    std::fs::create_dir_all(&dir)
                        .map_err(|e| IndexError::IoError(format!("Failed to create index directory: {}", e)))?;
                }
                
                // Open the mmap directory
                let dir = MmapDirectory::open(&dir)
                    .map_err(|e| IndexError::TantivyError(format!("Failed to open index directory: {}", e)))?;
                
                // Open or create the index
                TantivyIndex::open_or_create(dir, schema.clone())
                    .map_err(|e| IndexError::TantivyError(format!("Failed to create index: {}", e)))?
            },
            None => {
                // Create in-memory index
                TantivyIndex::create_in_ram(schema.clone())
            }
        };
        
        // Create writer
        let writer = index.writer(50_000_000).map_err(|e| {
            IndexError::CreateError(format!("Failed to create index writer: {}", e))
        })?;
        
        Ok(Self {
            field_name: name.to_string(),
            tantivy_index: index,
            field: data_field,
            id_field,
            writer: Arc::new(RwLock::new(writer)),
            hash_index: DashMap::new(),
            schema,
        })
    }
    
    /// Add a document to the index
    pub fn add_document(&self, document: &Document) -> Result<(), IndexError> {
        // Create a tantivy document
        let mut tantivy_doc = TantivyDoc::default();
        
        // Add the ID
        tantivy_doc.add_text(self.id_field, &document.id);
        
        // Add the field value if it exists in the document
        if let Some(field_value) = document.data.get(&self.field_name) {
            // Create a JSON object to add to the document
            if let Value::Object(map) = field_value {
                // If already an object, we can add it directly
                tantivy_doc.add_json_object(self.field, map.clone());
            } else {
                // For non-object values, wrap them in a simple object
                let mut simple_map = Map::new();
                simple_map.insert("value".to_string(), field_value.clone());
                tantivy_doc.add_json_object(self.field, simple_map);
            }
        }
        
        // Index the document
        let writer = self.writer.write().map_err(|e| {
            IndexError::IndexingError(format!("Failed to get writer lock: {}", e))
        })?;
        
        writer.add_document(tantivy_doc).map_err(|e| {
            IndexError::IndexingError(format!("Failed to add document: {}", e))
        })?;
        
        Ok(())
    }
    
    /// Remove a document from the index
    fn remove_from_index(&self, doc_id: &str, field_value: &serde_json::Value) -> Result<(), IndexError> {
        // Remove from hash index
        if let Some(str_value) = field_value.as_str() {
            if let Some(mut entry) = self.hash_index.get_mut(str_value) {
                entry.remove(doc_id);
            }
        } else if let Some(num_value) = field_value.as_f64() {
            let str_value = num_value.to_string();
            if let Some(mut entry) = self.hash_index.get_mut(&str_value) {
                entry.remove(doc_id);
            }
        } else if let Some(bool_value) = field_value.as_bool() {
            let str_value = bool_value.to_string();
            if let Some(mut entry) = self.hash_index.get_mut(&str_value) {
                entry.remove(doc_id);
            }
        }
        
        // Remove from tantivy index
        let mut writer = self.writer.write().map_err(|e| {
            IndexError::IndexingError(format!("Failed to acquire writer lock: {}", e))
        })?;
        
        writer.delete_term(Term::from_field_text(self.id_field, doc_id));
        
        Ok(())
    }
    
    /// Search the index for documents matching the query condition
    fn search(&self, operator: &QueryOperator, value: &serde_json::Value) -> Result<HashSet<String>, IndexError> {
        match operator {
            QueryOperator::Eq => {
                // Use hash index for equality
                if let Some(str_value) = value.as_str() {
                    if let Some(ids) = self.hash_index.get(str_value) {
                        return Ok(ids.clone());
                    }
                } else if let Some(num_value) = value.as_f64() {
                    let str_value = num_value.to_string();
                    if let Some(ids) = self.hash_index.get(&str_value) {
                        return Ok(ids.clone());
                    }
                } else if let Some(bool_value) = value.as_bool() {
                    let str_value = bool_value.to_string();
                    if let Some(ids) = self.hash_index.get(&str_value) {
                        return Ok(ids.clone());
                    }
                }
                Ok(HashSet::new())
            },
            QueryOperator::Contains => {
                // Use tantivy for text search
                if let Some(query_str) = value.as_str() {
                    let reader = self.tantivy_index
                        .reader_builder()
                        .reload_policy(ReloadPolicy::OnCommit)
                        .try_into().map_err(|e| {
                            IndexError::SearchError(format!("Failed to create reader: {}", e))
                        })?;
                    
                    let searcher = reader.searcher();
                    let query_parser = QueryParser::for_index(&self.tantivy_index, vec![self.field]);
                    
                    let query = query_parser.parse_query(query_str).map_err(|e| {
                        IndexError::SearchError(format!("Failed to parse query: {}", e))
                    })?;
                    
                    let top_docs = searcher.search(&query, &TopDocs::with_limit(1000)).map_err(|e| {
                        IndexError::SearchError(format!("Failed to search: {}", e))
                    })?;
                    
                    let mut result = HashSet::new();
                    for (_score, doc_address) in top_docs {
                        let doc = searcher.doc(doc_address).map_err(|e| {
                            IndexError::SearchError(format!("Failed to fetch document: {}", e))
                        })?;
                        
                        if let Some(id_value) = doc.get_first(self.id_field) {
                            if let Some(id_str) = id_value.as_text() {
                                result.insert(id_str.to_string());
                            }
                        }
                    }
                    
                    Ok(result)
                } else {
                    Ok(HashSet::new())
                }
            },
            // Other operators would be implemented similarly
            _ => Ok(HashSet::new()),
        }
    }
    
    /// Commit changes to the index
    fn commit(&self) -> Result<(), IndexError> {
        let mut writer = self.writer.write().map_err(|e| {
            IndexError::IndexingError(format!("Failed to acquire writer lock: {}", e))
        })?;
        
        writer.commit().map_err(|e| {
            IndexError::IndexingError(format!("Failed to commit index changes: {}", e))
        })?;
        
        Ok(())
    }

    fn get_schema_info(&self) -> Result<Field, IndexError> {
        self.schema.get_field("id").ok_or_else(|| {
            IndexError::SearchError("ID field not found in schema".to_string())
        })
    }
}

/// Manages indexes for a collection
pub struct IndexManager {
    /// Collection name
    collection_name: String,
    /// Field indexes
    field_indexes: DashMap<String, Arc<FieldIndex>>,
    /// Data directory for persistent indexes
    data_dir: Option<PathBuf>,
}

impl IndexManager {
    /// Create a new index manager
    pub fn new(collection_name: String) -> Self {
        Self {
            collection_name,
            field_indexes: DashMap::new(),
            data_dir: None,
        }
    }
    
    /// Create a new index manager with data directory
    pub fn with_data_dir(collection_name: String, data_dir: PathBuf) -> Self {
        Self {
            collection_name,
            field_indexes: DashMap::new(),
            data_dir: Some(data_dir),
        }
    }
    
    /// Get or create an index for a field
    fn get_or_create_field_index(&self, field_name: &str) -> Result<Arc<FieldIndex>, IndexError> {
        if let Some(index) = self.field_indexes.get(field_name) {
            Ok(index.clone())
        } else {
            let index = Arc::new(FieldIndex::new(
                field_name,
                self.data_dir.clone().map(|d| d.join(&self.collection_name))
            )?);
            
            self.field_indexes.insert(field_name.to_string(), index.clone());
            Ok(index)
        }
    }
    
    /// Index a document
    pub async fn index_document(&self, document: &Document) -> Result<(), IndexError> {
        if let serde_json::Value::Object(obj) = &document.data {
            for (field_name, value) in obj {
                let index = self.get_or_create_field_index(field_name)?;
                index.add_document(document)?;
            }
        }
        
        Ok(())
    }
    
    /// Update document index
    pub async fn update_document_index(
        &self,
        old_doc: &Document,
        new_doc: &Document
    ) -> Result<(), IndexError> {
        // First remove the old document
        self.remove_document(old_doc).await?;
        
        // Then add the new one
        self.index_document(new_doc).await?;
        
        Ok(())
    }
    
    /// Remove a document from indexes
    pub async fn remove_document(&self, document: &Document) -> Result<(), IndexError> {
        if let serde_json::Value::Object(obj) = &document.data {
            for (field_name, value) in obj {
                if let Some(index) = self.field_indexes.get(field_name) {
                    index.remove_from_index(&document.id, value)?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Execute a query against the indexes
    pub async fn query(&self, query: &Query) -> Result<Vec<String>, IndexError> {
        let mut result_sets: Vec<HashSet<String>> = Vec::new();
        
        for condition in &query.conditions {
            let field_name = &condition.field;
            
            // For nested fields, we need to extract the top-level field
            let top_field = field_name.split('.').next().unwrap_or(field_name);
            
            if let Some(index) = self.field_indexes.get(top_field) {
                let matches = index.search(&condition.operator, &condition.value)?;
                
                if !matches.is_empty() {
                    result_sets.push(matches);
                }
            }
        }
        
        // Intersect all result sets (AND semantics)
        let mut final_results = Vec::new();
        
        if !result_sets.is_empty() {
            // Start with the first set
            let mut intersection = result_sets[0].clone();
            
            // Intersect with each subsequent set
            for set in result_sets.iter().skip(1) {
                intersection = intersection.intersection(set).cloned().collect();
            }
            
            final_results.extend(intersection);
        }
        
        Ok(final_results)
    }
    
    /// Commit all pending index changes
    pub async fn commit(&self) -> Result<(), IndexError> {
        for index_entry in self.field_indexes.iter() {
            index_entry.commit()?;
        }
        
        Ok(())
    }
} 