mod test_utils;

use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode, Method},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;

use zachdb::api::{create_api_router, AppState};
use zachdb::core::{Database, Document};
use test_utils::{create_test_db, setup_test_collection, cleanup_temp_data_dir};

// Helper to create a test app with populated data
async fn setup_test_app(collection_name: &str, doc_count: usize) -> (Router, Arc<Database>, std::path::PathBuf) {
    let (db, data_dir) = create_test_db().await;
    
    if doc_count > 0 {
        setup_test_collection(&db, collection_name, doc_count).await;
    } else {
        // Just create an empty collection
        db.create_collection(collection_name).expect("Failed to create collection");
    }
    
    let app_state = Arc::new(AppState { db: db.clone() });
    let app = create_api_router(app_state);
    
    (app, db, data_dir)
}

#[tokio::test]
async fn test_server_info() {
    let (db, data_dir) = create_test_db().await;
    let app_state = Arc::new(AppState { db: db.clone() });
    let app = create_api_router(app_state);
    
    // Make request to root
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    
    // Check status and body
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(body["success"], json!(true));
    assert!(body["data"].is_object());
    assert_eq!(body["data"]["name"], json!("ZachDB"));
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_list_collections() {
    let (app, db, data_dir) = setup_test_app("test_collection", 0).await;
    
    // Create a second collection
    db.create_collection("another_collection").expect("Failed to create collection");
    
    // Make request to list collections
    let response = app
        .oneshot(Request::builder().uri("/collections").body(Body::empty()).unwrap())
        .await
        .unwrap();
    
    // Check status and body
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(body["success"], json!(true));
    assert!(body["data"].is_array());
    
    let collections = body["data"].as_array().unwrap();
    assert_eq!(collections.len(), 2);
    
    // Verify collection names
    let names: Vec<&str> = collections
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    
    assert!(names.contains(&"test_collection"));
    assert!(names.contains(&"another_collection"));
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_create_collection() {
    let (db, data_dir) = create_test_db().await;
    let app_state = Arc::new(AppState { db: db.clone() });
    let app = create_api_router(app_state);
    
    // Create a collection via API
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/collections")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"name":"api_collection"}"#))
                .unwrap()
        )
        .await
        .unwrap();
    
    // Check status
    assert_eq!(response.status(), StatusCode::CREATED);
    
    // Verify the collection exists
    let collections = db.list_collections();
    assert!(collections.contains(&"api_collection".to_string()));
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_create_document() {
    let (app, db, data_dir) = setup_test_app("test_collection", 0).await;
    
    // Create a document via API
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/collections/test_collection/documents")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"data":{"name":"API Document","value":100}}"#))
                .unwrap()
        )
        .await
        .unwrap();
    
    // Check status
    assert_eq!(response.status(), StatusCode::CREATED);
    
    // Parse response to get document ID
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(body["success"], json!(true));
    
    let doc_id = body["data"]["id"].as_str().unwrap();
    
    // Verify the document exists and has correct data
    let document = db.get_document("test_collection", doc_id)
        .expect("Failed to get document");
    
    assert_eq!(document.data["name"], json!("API Document"));
    assert_eq!(document.data["value"], json!(100));
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_get_document() {
    let (app, db, data_dir) = setup_test_app("test_collection", 1).await;
    
    // Get the document ID
    let documents = db.get_all_documents("test_collection")
        .expect("Failed to get documents");
    let doc_id = &documents[0].id;
    
    // Get the document via API
    let uri = format!("/collections/test_collection/documents/{}", doc_id);
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    // Check status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse response
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["data"]["id"], json!(doc_id));
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_update_document() {
    let (app, db, data_dir) = setup_test_app("test_collection", 1).await;
    
    // Get the document ID
    let documents = db.get_all_documents("test_collection")
        .expect("Failed to get documents");
    let doc_id = &documents[0].id;
    
    // Update the document via API
    let uri = format!("/collections/test_collection/documents/{}", doc_id);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(uri)
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"data":{"name":"Updated via API","value":200}}"#))
                .unwrap()
        )
        .await
        .unwrap();
    
    // Check status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Verify the document was updated
    let document = db.get_document("test_collection", doc_id)
        .expect("Failed to get document");
    
    assert_eq!(document.data["name"], json!("Updated via API"));
    assert_eq!(document.data["value"], json!(200));
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_delete_document() {
    let (app, db, data_dir) = setup_test_app("test_collection", 1).await;
    
    // Get the document ID
    let documents = db.get_all_documents("test_collection")
        .expect("Failed to get documents");
    let doc_id = &documents[0].id;
    
    // Delete the document via API
    let uri = format!("/collections/test_collection/documents/{}", doc_id);
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri(uri)
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    // Check status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Verify the document is gone
    assert!(db.get_document("test_collection", doc_id).is_err());
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_query_documents() {
    let (app, _db, data_dir) = setup_test_app("test_collection", 20).await;
    
    // Query documents via API
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/collections/test_collection/query")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{
                    "conditions": [
                        {"field": "active", "operator": "eq", "value": true},
                        {"field": "score", "operator": "gt", "value": 10.0}
                    ],
                    "limit": 5,
                    "sort_by": "score",
                    "sort_direction": "desc"
                }"#))
                .unwrap()
        )
        .await
        .unwrap();
    
    // Check status
    assert_eq!(response.status(), StatusCode::OK);
    
    // Parse response
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(body["success"], json!(true));
    
    let results = body["data"].as_array().unwrap();
    assert!(results.len() <= 5);
    
    // Verify results match query
    let mut last_score = f64::MAX;
    for doc in results {
        // Parse the document data which is stored as a JSON string
        let doc_data: Value = serde_json::from_str(doc["data"].as_str().unwrap()).unwrap();
        
        assert_eq!(doc_data["active"], json!(true));
        assert!(doc_data["score"].as_f64().unwrap() > 10.0);
        
        // Verify descending order
        let score = doc_data["score"].as_f64().unwrap();
        assert!(score <= last_score);
        last_score = score;
    }
    
    cleanup_temp_data_dir(&data_dir);
}

#[tokio::test]
async fn test_error_handling() {
    let (app, _db, data_dir) = setup_test_app("test_collection", 1).await;
    
    // 1. Test non-existent collection
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri("/collections/nonexistent/documents")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    // 2. Test non-existent document
    let response = app.clone()
        .oneshot(
            Request::builder()
                .uri("/collections/test_collection/documents/nonexistent")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    
    // 3. Test bad query
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/collections/test_collection/query")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{
                    "conditions": [
                        {"field": "active", "operator": "invalid", "value": true}
                    ]
                }"#))
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    
    cleanup_temp_data_dir(&data_dir);
} 