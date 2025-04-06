use dashmap::DashMap;
use std::sync::Arc;

use crate::core::types::{Document, Query, DbResult, DbError};
use crate::indexing::index::{Index, IndexManager};

/// Represents a collection of documents in the database
pub struct Collection {
    /// The name of the collection
    name: String,
    /// Thread-safe concurrent map of documents
    documents: Arc<DashMap<String, Document>>,
    /// Index manager for this collection
    index_manager: Arc<IndexManager>,
}

impl Collection {
    /// Creates a new collection with the given name
    pub fn new(name: String) -> Self {
        Collection {
            name: name.clone(),
            documents: Arc::new(DashMap::new()),
            index_manager: Arc::new(IndexManager::new(name)),
        }
    }
    
    /// Returns the name of the collection
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Returns the number of documents in the collection
    pub fn count(&self) -> usize {
        self.documents.len()
    }
    
    /// Inserts a document into the collection
    pub async fn insert(&self, document: Document) -> DbResult<String> {
        let id = document.id.clone();
        
        // Add to indexes first
        self.index_manager.index_document(&document).await
            .map_err(|e| DbError::IndexError(e.to_string()))?;
        
        // Then store the document
        self.documents.insert(id.clone(), document);
        
        Ok(id)
    }
    
    /// Retrieves a document by ID
    pub fn get(&self, id: &str) -> DbResult<Document> {
        self.documents.get(id)
            .map(|doc| doc.clone())
            .ok_or_else(|| DbError::DocumentNotFound(id.to_string()))
    }
    
    /// Updates a document
    pub async fn update(&self, id: &str, data: serde_json::Value) -> DbResult<Document> {
        let mut entry = self.documents.get_mut(id)
            .ok_or_else(|| DbError::DocumentNotFound(id.to_string()))?;
        
        // Create updated document (but don't change entry yet)
        let mut updated_doc = entry.clone();
        updated_doc.update(data);
        
        // Update indexes
        self.index_manager.update_document_index(&entry, &updated_doc).await
            .map_err(|e| DbError::IndexError(e.to_string()))?;
        
        // Now update the actual document
        entry.update(updated_doc.data.clone());
        
        Ok(entry.clone())
    }
    
    /// Deletes a document
    pub async fn delete(&self, id: &str) -> DbResult<Document> {
        // Get the document first
        let document = self.get(id)?;
        
        // Remove from index
        self.index_manager.remove_document(&document).await
            .map_err(|e| DbError::IndexError(e.to_string()))?;
        
        // Then remove from storage
        self.documents.remove(id)
            .map(|(_, doc)| doc)
            .ok_or_else(|| DbError::DocumentNotFound(id.to_string()))
    }
    
    /// Queries documents in the collection
    pub async fn query(&self, query: Query) -> DbResult<Vec<Document>> {
        // Use the index manager for the query if possible
        let doc_ids = self.index_manager.query(&query).await
            .map_err(|e| DbError::InvalidQuery(e.to_string()))?;
        
        let mut results = Vec::new();
        
        // If we have specific document IDs from the index, fetch only those
        if !doc_ids.is_empty() {
            for id in doc_ids {
                if let Some(doc) = self.documents.get(&id) {
                    results.push(doc.clone());
                }
            }
        } else {
            // Fallback to scanning all documents (should be rare if indexes are working properly)
            for entry in self.documents.iter() {
                if self.matches_query(&entry.value(), &query) {
                    results.push(entry.value().clone());
                }
            }
        }
        
        // Apply sorting if specified
        let sort_by = query.sort_by.as_ref();
        if let Some((field, ascending)) = sort_by {
            // Sort the documents
            results.sort_by(|a, b| {
                let a_value = a.data.get(field);
                let b_value = b.data.get(field);
                
                match (a_value, b_value) {
                    (Some(a_val), Some(b_val)) => {
                        if *ascending {
                            compare_json_values(a_val, b_val)
                        } else {
                            compare_json_values(b_val, a_val)
                        }
                    },
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
        
        // Apply skip
        if let Some(skip) = query.skip {
            if skip < results.len() {
                results = results[skip..].to_vec();
            } else {
                results = Vec::new();
            }
        }
        
        // Apply limit
        if let Some(limit) = query.limit {
            if limit < results.len() {
                results.truncate(limit);
            }
        }
        
        Ok(results)
    }
    
    /// Checks if a document matches the given query
    fn matches_query(&self, doc: &Document, query: &Query) -> bool {
        for condition in &query.conditions {
            let doc_value = extract_value_from_path(&doc.data, &condition.field);
            
            if !matches_condition(&doc_value, &condition.operator, &condition.value) {
                return false;
            }
        }
        
        true
    }
    
    /// Gets all documents in the collection
    pub fn get_all(&self) -> Vec<Document> {
        self.documents.iter()
            .map(|entry| entry.value().clone())
            .collect()
    }
}

// Utility function to extract a value from a nested path (e.g., "user.address.city")
fn extract_value_from_path<'a>(data: &'a serde_json::Value, path: &str) -> &'a serde_json::Value {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = data;
    
    for part in parts {
        if let serde_json::Value::Object(map) = current {
            if let Some(value) = map.get(part) {
                current = value;
            } else {
                return &serde_json::Value::Null;
            }
        } else {
            return &serde_json::Value::Null;
        }
    }
    
    current
}

// Helper function to compare JSON values for sorting
fn compare_json_values(a: &serde_json::Value, b: &serde_json::Value) -> std::cmp::Ordering {
    match (a, b) {
        (serde_json::Value::Number(a_num), serde_json::Value::Number(b_num)) => {
            if let (Some(a_f64), Some(b_f64)) = (a_num.as_f64(), b_num.as_f64()) {
                a_f64.partial_cmp(&b_f64).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                std::cmp::Ordering::Equal
            }
        },
        (serde_json::Value::String(a_str), serde_json::Value::String(b_str)) => {
            a_str.cmp(b_str)
        },
        (serde_json::Value::Bool(a_bool), serde_json::Value::Bool(b_bool)) => {
            a_bool.cmp(b_bool)
        },
        // Handle other cases or mixed types
        _ => std::cmp::Ordering::Equal,
    }
}

// Helper function to check if a value matches a condition with the given operator
fn matches_condition(
    doc_value: &serde_json::Value,
    operator: &crate::core::types::QueryOperator,
    query_value: &serde_json::Value,
) -> bool {
    use crate::core::types::QueryOperator;
    
    match operator {
        QueryOperator::Eq => doc_value == query_value,
        QueryOperator::Ne => doc_value != query_value,
        QueryOperator::Gt => {
            if let (Some(doc_num), Some(query_num)) = (doc_value.as_f64(), query_value.as_f64()) {
                doc_num > query_num
            } else if let (Some(doc_str), Some(query_str)) = (doc_value.as_str(), query_value.as_str()) {
                doc_str > query_str
            } else {
                false
            }
        },
        QueryOperator::Gte => {
            if let (Some(doc_num), Some(query_num)) = (doc_value.as_f64(), query_value.as_f64()) {
                doc_num >= query_num
            } else if let (Some(doc_str), Some(query_str)) = (doc_value.as_str(), query_value.as_str()) {
                doc_str >= query_str
            } else {
                false
            }
        },
        QueryOperator::Lt => {
            if let (Some(doc_num), Some(query_num)) = (doc_value.as_f64(), query_value.as_f64()) {
                doc_num < query_num
            } else if let (Some(doc_str), Some(query_str)) = (doc_value.as_str(), query_value.as_str()) {
                doc_str < query_str
            } else {
                false
            }
        },
        QueryOperator::Lte => {
            if let (Some(doc_num), Some(query_num)) = (doc_value.as_f64(), query_value.as_f64()) {
                doc_num <= query_num
            } else if let (Some(doc_str), Some(query_str)) = (doc_value.as_str(), query_value.as_str()) {
                doc_str <= query_str
            } else {
                false
            }
        },
        QueryOperator::Contains => {
            if let (Some(doc_str), Some(query_str)) = (doc_value.as_str(), query_value.as_str()) {
                doc_str.contains(query_str)
            } else {
                false
            }
        },
        QueryOperator::Exists => {
            !doc_value.is_null()
        },
        QueryOperator::In => {
            if let serde_json::Value::Array(array) = query_value {
                array.contains(doc_value)
            } else {
                false
            }
        },
    }
}
