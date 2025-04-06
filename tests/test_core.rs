mod test_utils;

use std::sync::Arc;
use serde_json::json;

use zachdb::core::{Database, Document, Query};
use zachdb::core::types::{QueryOperator, QueryCondition};
use test_utils::{create_test_db, create_test_documents, cleanup_temp_data_dir, setup_test_collection};

#[tokio::test]
async fn test_create_collection() {
    let (db, data_dir) = create_test_db().await;
    
    // Create a collection
    let collection_name = "test_collection";
    let collection = db.create_collection(collection_name).expect("Failed to create collection");
    
    // Verify the collection was created
    assert_eq!(collection.name(), collection_name);
    assert_eq!(collection.count(), 0);
    
    // Verify the collection is in the list
    let collections = db.list_collections();
    assert!(collections.contains(&collection_name.to_string()));
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_delete_collection() {
    let (db, data_dir) = create_test_db().await;
    
    // Create a collection
    let collection_name = "test_collection";
    db.create_collection(collection_name).expect("Failed to create collection");
    
    // Delete the collection
    db.delete_collection(collection_name).expect("Failed to delete collection");
    
    // Verify the collection is gone
    let collections = db.list_collections();
    assert!(!collections.contains(&collection_name.to_string()));
    
    // Verify getting the collection fails
    assert!(db.get_collection(collection_name).is_err());
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_insert_document() {
    let (db, data_dir) = create_test_db().await;
    
    // Create a collection
    let collection_name = "test_collection";
    db.create_collection(collection_name).expect("Failed to create collection");
    
    // Create and insert a document
    let data = json!({
        "name": "Test Document",
        "value": 42,
        "tags": ["test", "example"]
    });
    
    let document = Document::new(data.clone());
    let doc_id = document.id.clone();
    
    db.insert_document(collection_name, document.clone())
        .await
        .expect("Failed to insert document");
    
    // Verify the document was inserted
    let retrieved = db.get_document(collection_name, &doc_id)
        .expect("Failed to get document");
    
    assert_eq!(retrieved.id, doc_id);
    assert_eq!(retrieved.data, data);
    
    // Verify collection count
    let collection = db.get_collection(collection_name)
        .expect("Failed to get collection");
    assert_eq!(collection.count(), 1);
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_update_document() {
    let (db, data_dir) = create_test_db().await;
    
    // Create a collection with a document
    let collection_name = "test_collection";
    db.create_collection(collection_name).expect("Failed to create collection");
    
    // Create and insert a document
    let data = json!({
        "name": "Original Name",
        "value": 42
    });
    
    let document = Document::new(data);
    let doc_id = document.id.clone();
    
    db.insert_document(collection_name, document)
        .await
        .expect("Failed to insert document");
    
    // Update the document
    let updated_data = json!({
        "name": "Updated Name",
        "value": 100,
        "new_field": "New Value"
    });
    
    let updated = db.update_document(collection_name, &doc_id, updated_data.clone())
        .await
        .expect("Failed to update document");
    
    // Verify the document was updated
    assert_eq!(updated.id, doc_id);
    assert_eq!(updated.data, updated_data);
    
    let retrieved = db.get_document(collection_name, &doc_id)
        .expect("Failed to get document");
    
    assert_eq!(retrieved.id, doc_id);
    assert_eq!(retrieved.data, updated_data);
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_delete_document() {
    let (db, data_dir) = create_test_db().await;
    
    // Set up a collection with a document
    let collection_name = "test_collection";
    db.create_collection(collection_name).expect("Failed to create collection");
    
    let document = Document::new(json!({"name": "Test Document"}));
    let doc_id = document.id.clone();
    
    db.insert_document(collection_name, document)
        .await
        .expect("Failed to insert document");
    
    // Delete the document
    let deleted = db.delete_document(collection_name, &doc_id)
        .await
        .expect("Failed to delete document");
    
    assert_eq!(deleted.id, doc_id);
    
    // Verify the document is gone
    assert!(db.get_document(collection_name, &doc_id).is_err());
    
    // Verify collection count
    let collection = db.get_collection(collection_name)
        .expect("Failed to get collection");
    assert_eq!(collection.count(), 0);
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_query_documents() {
    let (db, data_dir) = create_test_db().await;
    
    // Set up a collection with multiple documents
    let collection_name = "test_collection";
    let documents = setup_test_collection(&db, collection_name, 20).await;
    
    // Test various queries
    
    // 1. Equality query
    let eq_query = Query {
        conditions: vec![
            QueryCondition {
                field: "active".to_string(),
                operator: QueryOperator::Eq,
                value: json!(true),
            }
        ],
        limit: None,
        skip: None,
        sort_by: None,
    };
    
    let results = db.query_documents(collection_name, eq_query)
        .await
        .expect("Failed to query documents");
    
    assert_eq!(results.len(), 10);
    for doc in &results {
        assert_eq!(doc.data["active"], json!(true));
    }
    
    // 2. Greater than query
    let gt_query = Query {
        conditions: vec![
            QueryCondition {
                field: "score".to_string(),
                operator: QueryOperator::Gt,
                value: json!(15.0),
            }
        ],
        limit: None,
        skip: None,
        sort_by: None,
    };
    
    let results = db.query_documents(collection_name, gt_query)
        .await
        .expect("Failed to query documents");
    
    assert!(!results.is_empty());
    for doc in &results {
        assert!(doc.data["score"].as_f64().unwrap() > 15.0);
    }
    
    // 3. Combined query with limit and sort
    let combined_query = Query {
        conditions: vec![
            QueryCondition {
                field: "active".to_string(),
                operator: QueryOperator::Eq,
                value: json!(true),
            },
            QueryCondition {
                field: "score".to_string(),
                operator: QueryOperator::Gt,
                value: json!(5.0),
            }
        ],
        limit: Some(5),
        skip: None,
        sort_by: Some(("score".to_string(), false)), // descending
    };
    
    let results = db.query_documents(collection_name, combined_query)
        .await
        .expect("Failed to query documents");
    
    assert!(results.len() <= 5);
    
    // Verify sorting and conditions
    let mut last_score = f64::MAX;
    for doc in &results {
        assert_eq!(doc.data["active"], json!(true));
        assert!(doc.data["score"].as_f64().unwrap() > 5.0);
        
        // Verify descending order
        let score = doc.data["score"].as_f64().unwrap();
        assert!(score <= last_score);
        last_score = score;
    }
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_persistence() {
    let data_dir = test_utils::create_temp_data_dir();
    
    // Make sure the directory is empty
    if data_dir.exists() {
        let _ = std::fs::remove_dir_all(&data_dir);
        std::fs::create_dir_all(&data_dir).expect("Failed to recreate data directory");
    }
    
    // Create DB and add data
    {
        let db = Database::new("test-persist".to_string(), data_dir.clone())
            .await
            .expect("Failed to create test database");
        
        let db = Arc::new(db);
        
        // Create a collection
        let collection_name = "persist_collection";
        db.create_collection(collection_name).expect("Failed to create collection");
        
        // Add some documents
        for i in 0..5 {
            let doc = Document::new(json!({
                "index": i,
                "name": format!("Persist Test {}", i)
            }));
            
            db.insert_document(collection_name, doc)
                .await
                .expect("Failed to insert document");
        }
        
        // Let DB be dropped here
    }
    
    // Create a new DB instance with the same data dir
    {
        let db = Database::new("test-persist".to_string(), data_dir.clone())
            .await
            .expect("Failed to create test database");
        
        let db = Arc::new(db);
        
        // Verify collection exists
        let collections = db.list_collections();
        assert!(collections.contains(&"persist_collection".to_string()));
        
        // Verify documents were loaded
        let collection = db.get_collection("persist_collection")
            .expect("Failed to get collection");
        
        assert_eq!(collection.count(), 5);
        
        // Verify document content
        let docs = db.get_all_documents("persist_collection")
            .expect("Failed to get all documents");
        
        for doc in docs {
            let index = doc.data["index"].as_i64().unwrap();
            assert_eq!(doc.data["name"], json!(format!("Persist Test {}", index)));
        }
    }
    
    cleanup_temp_data_dir(&data_dir);
} 