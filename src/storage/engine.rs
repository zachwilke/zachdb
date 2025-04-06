use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write, BufReader, BufWriter};
use std::path::{Path, PathBuf};

use bincode;
use serde::{Serialize, Deserialize};

use crate::core::types::Document;

/// Error type for storage operations
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {message}")]
    IoError { message: String },
    
    #[error("Serialization error: {message}")]
    SerializationError { message: String },
    
    #[error("Deserialization error: {message}")]
    DeserializationError { message: String },
    
    #[error("Collection not found: {name}")]
    CollectionNotFound { name: String },
    
    #[error("Collection already exists: {name}")]
    CollectionExists { name: String },
    
    #[error("Document not found: {id}")]
    DocumentNotFound { id: String },
}

impl From<io::Error> for StorageError {
    fn from(err: io::Error) -> Self {
        StorageError::IoError { message: err.to_string() }
    }
}

/// Storage engine for persistent document storage
pub struct StorageEngine {
    /// Base directory for database files
    base_dir: PathBuf,
}

impl StorageEngine {
    /// Create a new storage engine with the given data directory
    pub fn new(base_dir: PathBuf) -> Result<Self, StorageError> {
        // Ensure data directory exists
        fs::create_dir_all(&base_dir)?;
        
        // Create collections directory if it doesn't exist
        let collections_dir = base_dir.join("collections");
        fs::create_dir_all(&collections_dir)?;
        
        Ok(Self {
            base_dir,
        })
    }
    
    /// Get the path for a collection
    fn collection_path(&self, collection_name: &str) -> PathBuf {
        self.base_dir.join("collections").join(collection_name)
    }
    
    /// Get the path for a document
    fn document_path(&self, collection_name: &str, document_id: &str) -> PathBuf {
        self.collection_path(collection_name).join(format!("{}.bin", document_id))
    }
    
    /// Create a new collection
    pub fn create_collection(&self, name: &str) -> Result<(), StorageError> {
        let collection_path = self.collection_path(name);
        
        if collection_path.exists() {
            return Err(StorageError::CollectionExists { name: name.to_string() });
        }
        
        fs::create_dir_all(&collection_path)?;
        
        // Initialize collection metadata
        let metadata = CollectionMetadata {
            name: name.to_string(),
            document_count: 0,
            created_at: chrono::Utc::now().timestamp(),
        };
        
        self.store_collection_metadata(name, &metadata)?;
        
        Ok(())
    }
    
    /// Store collection metadata
    fn store_collection_metadata(&self, name: &str, metadata: &CollectionMetadata) -> Result<(), StorageError> {
        let metadata_path = self.collection_path(name).join("metadata.bin");
        let file = File::create(metadata_path)?;
        let mut writer = BufWriter::new(file);
        
        let bytes = bincode::serialize(metadata)
            .map_err(|e| StorageError::SerializationError { message: e.to_string() })?;
        
        writer.write_all(&bytes)?;
        Ok(())
    }
    
    /// Load collection metadata
    fn load_collection_metadata(&self, name: &str) -> Result<CollectionMetadata, StorageError> {
        let metadata_path = self.collection_path(name).join("metadata.bin");
        
        if !metadata_path.exists() {
            return Err(StorageError::CollectionNotFound { name: name.to_string() });
        }
        
        let file = File::open(metadata_path)?;
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        
        bincode::deserialize(&buffer)
            .map_err(|e| StorageError::DeserializationError { message: e.to_string() })
    }
    
    /// Delete a collection
    pub fn delete_collection(&self, name: &str) -> Result<(), StorageError> {
        let collection_path = self.collection_path(name);
        
        if !collection_path.exists() {
            return Err(StorageError::CollectionNotFound { name: name.to_string() });
        }
        
        fs::remove_dir_all(collection_path)?;
        
        Ok(())
    }
    
    /// List all collections
    pub fn list_collections(&self) -> Result<Vec<String>, StorageError> {
        let collections_dir = self.base_dir.join("collections");
        
        if !collections_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut collections = Vec::new();
        
        for entry in fs::read_dir(collections_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    collections.push(name.to_string());
                }
            }
        }
        
        Ok(collections)
    }
    
    /// Store a document
    pub fn store_document(&self, collection_name: &str, document: &Document) -> Result<(), StorageError> {
        let collection_path = self.collection_path(collection_name);
        
        if !collection_path.exists() {
            return Err(StorageError::CollectionNotFound { name: collection_name.to_string() });
        }
        
        let document_path = self.document_path(collection_name, &document.id);
        
        self.store_document_to_file(document, &document_path)?;
        
        // Update metadata
        let mut metadata = self.load_collection_metadata(collection_name)?;
        metadata.document_count = self.count_documents(collection_name)?;
        self.store_collection_metadata(collection_name, &metadata)?;
        
        Ok(())
    }
    
    /// Delete a document
    pub fn delete_document(&self, collection_name: &str, document_id: &str) -> Result<(), StorageError> {
        let document_path = self.document_path(collection_name, document_id);
        
        if !document_path.exists() {
            return Err(StorageError::DocumentNotFound { id: document_id.to_string() });
        }
        
        fs::remove_file(document_path)?;
        
        // Update collection metadata
        let mut metadata = self.load_collection_metadata(collection_name)?;
        metadata.document_count = self.count_documents(collection_name)?;
        self.store_collection_metadata(collection_name, &metadata)?;
        
        Ok(())
    }
    
    /// Load a document
    pub fn load_document(&self, collection_name: &str, document_id: &str) -> Result<Document, StorageError> {
        let document_path = self.document_path(collection_name, document_id);
        
        if !document_path.exists() {
            return Err(StorageError::DocumentNotFound { id: document_id.to_string() });
        }
        
        self.load_document_from_file(&document_path)
    }
    
    /// Load all documents in a collection
    pub fn load_collection(&self, collection_name: &str) -> Result<Vec<Document>, StorageError> {
        let collection_path = self.collection_path(collection_name);
        
        if !collection_path.exists() {
            return Err(StorageError::CollectionNotFound { name: collection_name.to_string() });
        }
        
        let mut documents = Vec::new();
        
        for entry in fs::read_dir(&collection_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "bin") {
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                    // Skip metadata file
                    if filename == "metadata.bin" {
                        continue;
                    }
                    
                    let document = self.load_document_from_file(&path)?;
                    documents.push(document);
                }
            }
        }
        
        Ok(documents)
    }
    
    /// Count documents in a collection
    pub fn count_documents(&self, collection_name: &str) -> Result<usize, StorageError> {
        let collection_path = self.collection_path(collection_name);
        
        if !collection_path.exists() {
            return Err(StorageError::CollectionNotFound { name: collection_name.to_string() });
        }
        
        let mut count = 0;
        
        for entry in fs::read_dir(&collection_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "bin") {
                if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                    if filename != "metadata.bin" {
                        count += 1;
                    }
                }
            }
        }
        
        Ok(count)
    }
    
    /// Helper to load a document from a file
    fn load_document_from_file(&self, file_path: &Path) -> Result<Document, StorageError> {
        let file = File::open(file_path)?;
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        
        bincode::deserialize(&buffer)
            .map_err(|e| StorageError::DeserializationError { message: e.to_string() })
    }
    
    /// Helper to store a document to a file
    fn store_document_to_file(&self, document: &Document, file_path: &Path) -> Result<(), StorageError> {
        let file = File::create(file_path)?;
        let mut writer = BufWriter::new(file);
        
        let bytes = bincode::serialize(document)
            .map_err(|e| StorageError::SerializationError { message: e.to_string() })?;
        
        writer.write_all(&bytes)?;
        Ok(())
    }
    
    /// Backup the database to another location
    pub fn backup_database(&self, backup_path: PathBuf) -> Result<(), StorageError> {
        if !backup_path.exists() {
            fs::create_dir_all(&backup_path)?;
        }
        
        copy_dir_all(&self.base_dir, &backup_path)?;
        
        Ok(())
    }
}

/// Collection metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollectionMetadata {
    /// Collection name
    name: String,
    /// Number of documents in the collection
    document_count: usize,
    /// Collection creation timestamp
    created_at: i64,
}

// Helper function to recursively copy directories
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
} 