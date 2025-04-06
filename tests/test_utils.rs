use std::path::PathBuf;
use std::sync::Arc;
use std::fs;
use uuid::Uuid;
use serde_json::json;

use zachdb::core::{Database, Document};

/// Creates a temporary directory for test data
pub fn create_temp_data_dir() -> PathBuf {
    let temp_dir = std::env::temp_dir()
        .join(format!("zachdb-test-{}", Uuid::new_v4()));
    
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    
    temp_dir
}

/// Cleans up a temporary test directory
pub fn cleanup_temp_data_dir(path: &PathBuf) {
    if path.exists() && path.starts_with(std::env::temp_dir()) {
        let _ = fs::remove_dir_all(path);
    }
}

/// Creates a test database instance
pub async fn create_test_db() -> (Arc<Database>, PathBuf) {
    let data_dir = create_temp_data_dir();
    
    let db = Database::new("test-db".to_string(), data_dir.clone())
        .await
        .expect("Failed to create test database");
    
    (Arc::new(db), data_dir)
}

/// Creates test documents
pub fn create_test_documents(count: usize) -> Vec<Document> {
    let mut documents = Vec::with_capacity(count);
    
    for i in 0..count {
        let doc = Document::new(json!({
            "id": i,
            "name": format!("Test {}", i),
            "active": i % 2 == 0,
            "score": i as f64 * 1.5,
            "tags": vec![format!("tag{}", i), format!("category{}", i % 3)]
        }));
        
        documents.push(doc);
    }
    
    documents
}

/// Set up a test collection with documents
pub async fn setup_test_collection(
    db: &Arc<Database>, 
    collection_name: &str,
    doc_count: usize
) -> Vec<Document> {
    // Create collection
    db.create_collection(collection_name)
        .expect("Failed to create test collection");
    
    // Create documents
    let documents = create_test_documents(doc_count);
    
    // Insert documents
    for doc in &documents {
        db.insert_document(collection_name, doc.clone())
            .await
            .expect("Failed to insert test document");
    }
    
    documents
} 