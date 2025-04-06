use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Json, IntoResponse},
};
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::core::{Database, Document, DbError, Query};
use crate::core::types::{QueryOperator, QueryCondition};

/// API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    /// Whether the request was successful
    pub success: bool,
    /// Optional data for successful responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Optional error message for failed responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    /// Create a successful response with data
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
    
    /// Create an error response with a message
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// App state containing the database
pub struct AppState {
    pub db: Arc<Database>,
}

/// Collection creation request
#[derive(Deserialize)]
pub struct CreateCollectionRequest {
    pub name: String,
}

/// Collection response
#[derive(Serialize)]
pub struct CollectionResponse {
    pub name: String,
    pub document_count: usize,
}

/// Document creation request
#[derive(Deserialize)]
pub struct CreateDocumentRequest {
    pub data: Value,
}

/// Document update request
#[derive(Deserialize)]
pub struct UpdateDocumentRequest {
    pub data: Value,
}

/// Query request
#[derive(Deserialize)]
pub struct QueryRequest {
    pub conditions: Vec<QueryConditionRequest>,
    pub limit: Option<usize>,
    pub skip: Option<usize>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
}

/// Query condition request
#[derive(Deserialize)]
pub struct QueryConditionRequest {
    pub field: String,
    pub operator: String,
    pub value: Value,
}

/// List collections handler
pub async fn list_collections(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let collections = state.db.list_collections();
    
    let collection_responses: Vec<CollectionResponse> = collections
        .into_iter()
        .map(|name| {
            let count = match state.db.get_collection(&name) {
                Ok(col) => col.count(),
                Err(_) => 0,
            };
            
            CollectionResponse {
                name,
                document_count: count,
            }
        })
        .collect();
    
    (
        StatusCode::OK,
        Json(ApiResponse::success(collection_responses)),
    )
}

/// Create collection handler
pub async fn create_collection(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateCollectionRequest>,
) -> impl IntoResponse {
    match state.db.create_collection(&request.name) {
        Ok(_) => {
            let response = CollectionResponse {
                name: request.name,
                document_count: 0,
            };
            
            (
                StatusCode::CREATED,
                Json(ApiResponse::success(response)),
            )
        },
        Err(err) => {
            let status = match err {
                DbError::CollectionExists(_) => StatusCode::CONFLICT,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            
            (
                status,
                Json(ApiResponse::<CollectionResponse>::error(err.to_string())),
            )
        }
    }
}

/// Delete collection handler
pub async fn delete_collection(
    State(state): State<Arc<AppState>>,
    Path(collection_name): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_collection(&collection_name) {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::success(json!({ "deleted": collection_name }))),
        ),
        Err(err) => {
            let status = match err {
                DbError::CollectionNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            
            (
                status,
                Json(ApiResponse::<Value>::error(err.to_string())),
            )
        }
    }
}

/// Get all documents handler
pub async fn get_all_documents(
    State(state): State<Arc<AppState>>,
    Path(collection_name): Path<String>,
) -> impl IntoResponse {
    match state.db.get_all_documents(&collection_name) {
        Ok(documents) => (
            StatusCode::OK,
            Json(ApiResponse::success(documents)),
        ),
        Err(err) => {
            let status = match err {
                DbError::CollectionNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            
            (
                status,
                Json(ApiResponse::<Vec<Document>>::error(err.to_string())),
            )
        }
    }
}

/// Get document handler
pub async fn get_document(
    State(state): State<Arc<AppState>>,
    Path((collection_name, document_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.db.get_document(&collection_name, &document_id) {
        Ok(document) => (
            StatusCode::OK,
            Json(ApiResponse::success(document)),
        ),
        Err(err) => {
            let status = match err {
                DbError::CollectionNotFound(_) => StatusCode::NOT_FOUND,
                DbError::DocumentNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            
            (
                status,
                Json(ApiResponse::<Document>::error(err.to_string())),
            )
        }
    }
}

/// Create document handler
pub async fn create_document(
    State(state): State<Arc<AppState>>,
    Path(collection_name): Path<String>,
    Json(request): Json<CreateDocumentRequest>,
) -> impl IntoResponse {
    let document = Document::new(request.data);
    
    match state.db.insert_document(&collection_name, document.clone()).await {
        Ok(_id) => (
            StatusCode::CREATED,
            Json(ApiResponse::success(document)),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(format!("Failed to create document: {}", e))),
        ),
    }
}

/// Update document handler
pub async fn update_document(
    State(state): State<Arc<AppState>>,
    Path((collection_name, document_id)): Path<(String, String)>,
    Json(request): Json<UpdateDocumentRequest>,
) -> impl IntoResponse {
    match state.db.update_document(&collection_name, &document_id, request.data).await {
        Ok(document) => (
            StatusCode::OK,
            Json(ApiResponse::success(document)),
        ),
        Err(err) => {
            let status = match err {
                DbError::CollectionNotFound(_) => StatusCode::NOT_FOUND,
                DbError::DocumentNotFound(_) => StatusCode::NOT_FOUND,
                DbError::InvalidData(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            
            (
                status,
                Json(ApiResponse::<Document>::error(err.to_string())),
            )
        }
    }
}

/// Delete document handler
pub async fn delete_document(
    State(state): State<Arc<AppState>>,
    Path((collection_name, document_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.db.delete_document(&collection_name, &document_id).await {
        Ok(document) => (
            StatusCode::OK,
            Json(ApiResponse::success(document)),
        ),
        Err(err) => {
            let status = match err {
                DbError::CollectionNotFound(_) => StatusCode::NOT_FOUND,
                DbError::DocumentNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            
            (
                status,
                Json(ApiResponse::<Document>::error(err.to_string())),
            )
        }
    }
}

/// Query documents handler
pub async fn query_documents(
    State(state): State<Arc<AppState>>,
    Path(collection_name): Path<String>,
    Json(query): Json<QueryRequest>,
) -> impl IntoResponse {
    // Convert API query to DB query
    let mut conditions = Vec::new();
    
    for c in query.conditions {
        // Convert string operator to QueryOperator
        let operator = match c.operator.as_str() {
            "eq" => QueryOperator::Eq,
            "ne" => QueryOperator::Ne,
            "gt" => QueryOperator::Gt,
            "gte" => QueryOperator::Gte,
            "lt" => QueryOperator::Lt,
            "lte" => QueryOperator::Lte,
            "contains" => QueryOperator::Contains,
            "exists" => QueryOperator::Exists,
            "in" => QueryOperator::In,
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<Vec<Document>>::error(
                        format!("Invalid operator: {}", c.operator)
                    )),
                );
            }
        };
        
        conditions.push(QueryCondition {
            field: c.field,
            operator,
            value: c.value,
        });
    }
    
    let sort_by = if let (Some(field), Some(direction)) = (query.sort_by, query.sort_direction) {
        let ascending = direction == "asc";
        Some((field, ascending))
    } else {
        None
    };
    
    let db_query = Query {
        conditions,
        limit: query.limit,
        skip: query.skip,
        sort_by,
    };
    
    match state.db.query_documents(&collection_name, db_query).await {
        Ok(documents) => (
            StatusCode::OK,
            Json(ApiResponse::success(documents))
        ),
        Err(e) => {
            let status = match e {
                DbError::CollectionNotFound(_) => StatusCode::NOT_FOUND,
                DbError::InvalidQuery(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            
            (
                status,
                Json(ApiResponse::<Vec<Document>>::error(e.to_string()))
            )
        }
    }
}

/// Server info handler
pub async fn server_info(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let info = json!({
        "name": "ZachDB",
        "version": env!("CARGO_PKG_VERSION"),
        "collections": state.db.list_collections().len(),
    });
    
    (
        StatusCode::OK,
        Json(ApiResponse::success(info)),
    )
} 