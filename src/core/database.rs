use dashmap::DashMap;
use std::sync::Arc;
use std::path::PathBuf;

use crate::core::collection::Collection;
use crate::core::types::{DbResult, DbError, Document, Query};
use crate::storage::engine::StorageEngine;

/// The main database structure that manages collections
pub struct Database {
    /// Name of the database
    name: String,
    /// Map of collection name to collection instance
    collections: Arc<DashMap<String, Arc<Collection>>>,
    /// Storage engine for persistence
    storage: Arc<StorageEngine>,
    /// Data directory path
    data_dir: PathBuf,
}

impl Database {
    /// Creates a new database instance
    pub async fn new(name: String, data_dir: PathBuf) -> Result<Self, DbError> {
        // Create data directory if it doesn't exist
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| DbError::Io(format!("Failed to create data directory: {}", e)))?;
        }
        
        // Initialize storage engine
        let storage_result = StorageEngine::new(data_dir.clone());
        let storage = Arc::new(match storage_result {
            Ok(storage) => storage,
            Err(e) => return Err(DbError::Storage(format!("Failed to initialize storage engine: {}", e))),
        });
        
        let collections = Arc::new(DashMap::new());
        
        let db = Self {
            name,
            collections,
            storage,
            data_dir,
        };
        
        // Load existing collections from storage
        db.load_collections().await?;
        
        Ok(db)
    }
    
    /// Load collections from storage
    async fn load_collections(&self) -> DbResult<()> {
        let collection_names = self.storage.list_collections()
            .map_err(|e| DbError::Storage(e.to_string()))?;
        
        for name in collection_names {
            let collection = Arc::new(Collection::new(name.clone()));
            self.collections.insert(name.clone(), collection.clone());
            
            // Load documents for this collection
            let documents = self.storage.load_collection(&name)
                .map_err(|e| DbError::Storage(e.to_string()))?;
            
            for document in documents {
                collection.insert(document).await?;
            }
        }
        
        Ok(())
    }
    
    /// Returns the name of the database
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Creates a new collection
    pub fn create_collection(&self, name: &str) -> DbResult<Arc<Collection>> {
        if self.collections.contains_key(name) {
            return Err(DbError::CollectionExists(name.to_string()));
        }
        
        let collection = Arc::new(Collection::new(name.to_string()));
        self.collections.insert(name.to_string(), collection.clone());
        
        // Create the collection in storage
        self.storage.create_collection(name)
            .map_err(|e| DbError::Storage(e.to_string()))?;
        
        Ok(collection)
    }
    
    /// Returns a collection by name
    pub fn get_collection(&self, name: &str) -> DbResult<Arc<Collection>> {
        self.collections.get(name)
            .map(|c| c.clone())
            .ok_or_else(|| DbError::CollectionNotFound(name.to_string()))
    }
    
    /// Deletes a collection
    pub fn delete_collection(&self, name: &str) -> DbResult<()> {
        if !self.collections.contains_key(name) {
            return Err(DbError::CollectionNotFound(name.to_string()));
        }
        
        self.collections.remove(name);
        
        // Delete from storage
        self.storage.delete_collection(name)
            .map_err(|e| DbError::Storage(e.to_string()))?;
        
        Ok(())
    }
    
    /// Lists all collection names
    pub fn list_collections(&self) -> Vec<String> {
        self.collections.iter()
            .map(|entry| entry.key().clone())
            .collect()
    }
    
    /// Inserts a document into a collection
    pub async fn insert_document(&self, collection_name: &str, document: Document) -> DbResult<String> {
        let collection = self.get_collection(collection_name)?;
        let id = collection.insert(document.clone()).await?;
        
        // Store the document
        self.storage.store_document(collection_name, &document)
            .map_err(|e| DbError::Storage(e.to_string()))?;
        
        Ok(id)
    }
    
    /// Updates a document in a collection
    pub async fn update_document(&self, collection_name: &str, id: &str, data: serde_json::Value) -> DbResult<Document> {
        let collection = self.get_collection(collection_name)?;
        let updated = collection.update(id, data).await?;
        
        // Update in storage
        self.storage.store_document(collection_name, &updated)
            .map_err(|e| DbError::Storage(e.to_string()))?;
        
        Ok(updated)
    }
    
    /// Deletes a document from a collection
    pub async fn delete_document(&self, collection_name: &str, id: &str) -> DbResult<Document> {
        let collection = self.get_collection(collection_name)?;
        let document = collection.delete(id).await?;
        
        // Delete from storage
        self.storage.delete_document(collection_name, id)
            .map_err(|e| DbError::Storage(e.to_string()))?;
        
        Ok(document)
    }
    
    /// Gets a document from a collection
    pub fn get_document(&self, collection_name: &str, id: &str) -> DbResult<Document> {
        let collection = self.get_collection(collection_name)?;
        collection.get(id)
    }
    
    /// Queries documents in a collection
    pub async fn query_documents(&self, collection_name: &str, query: Query) -> DbResult<Vec<Document>> {
        let collection = self.get_collection(collection_name)?;
        collection.query(query).await
    }
    
    /// Gets all documents in a collection
    pub fn get_all_documents(&self, collection_name: &str) -> DbResult<Vec<Document>> {
        let collection = self.get_collection(collection_name)?;
        Ok(collection.get_all())
    }
    
    /// Performs a database backup
    pub fn backup(&self, backup_path: PathBuf) -> DbResult<()> {
        self.storage.backup_database(backup_path)
            .map_err(|e| DbError::Storage(e.to_string()))
    }
    
    /// Shuts down the database, ensuring all data is persisted
    pub async fn shutdown(&self) -> DbResult<()> {
        // Additional cleanup if needed
        Ok(())
    }
} 