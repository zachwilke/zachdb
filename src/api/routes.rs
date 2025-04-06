use std::sync::Arc;

use axum::{
    routing::{get, post, put, delete},
    Router,
};

use crate::api::handlers::{
    list_collections,
    create_collection,
    delete_collection,
    get_all_documents,
    get_document,
    create_document,
    update_document,
    delete_document,
    query_documents,
    server_info,
    AppState,
};

/// Creates the API router
pub fn create_api_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        // Server info
        .route("/", get(server_info))
        
        // Collections
        .route("/collections", get(list_collections))
        .route("/collections", post(create_collection))
        .route("/collections/:name", delete(delete_collection))
        
        // Documents
        .route("/collections/:name/documents", get(get_all_documents))
        .route("/collections/:name/documents", post(create_document))
        .route("/collections/:name/documents/:id", get(get_document))
        .route("/collections/:name/documents/:id", put(update_document))
        .route("/collections/:name/documents/:id", delete(delete_document))
        
        // Queries
        .route("/collections/:name/query", post(query_documents))
        
        .with_state(app_state)
} 