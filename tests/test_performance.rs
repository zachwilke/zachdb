mod test_utils;

use std::sync::Arc;
use std::time::{Instant, Duration};
use tokio::task;
use serde_json::json;
use futures::future;

use zachdb::core::{Database, Document, Query, QueryOperator, QueryCondition};
use test_utils::{create_test_db, cleanup_temp_data_dir};

#[tokio::test]
async fn test_concurrent_document_operations() {
    // This test verifies the database can handle many concurrent operations
    let (db, data_dir) = create_test_db().await;
    
    // Create a test collection
    let collection_name = "performance_test";
    db.create_collection(collection_name).expect("Failed to create collection");
    
    // Number of operations to perform
    let op_count = 100;
    
    // 1. Test concurrent inserts
    println!("Testing {} concurrent inserts...", op_count);
    
    let start = Instant::now();
    let mut insert_tasks = Vec::with_capacity(op_count);
    
    for i in 0..op_count {
        let db_clone = db.clone();
        let collection_name = collection_name.to_string();
        
        let task = task::spawn(async move {
            let doc = Document::new(json!({
                "index": i,
                "name": format!("Doc {}", i),
                "value": i % 100,
                "timestamp": chrono::Utc::now().timestamp()
            }));
            
            db_clone.insert_document(&collection_name, doc).await
        });
        
        insert_tasks.push(task);
    }
    
    future::join_all(insert_tasks).await;
    let insert_duration = start.elapsed();
    
    println!("Inserted {} documents in {:?} ({} ops/sec)", 
        op_count, 
        insert_duration,
        (op_count as f64 / insert_duration.as_secs_f64()) as u64
    );
    
    // Verify all documents were inserted
    let collection = db.get_collection(collection_name).unwrap();
    assert_eq!(collection.count(), op_count);
    
    // 2. Test concurrent reads
    println!("Testing {} concurrent reads...", op_count);
    
    // Get all document IDs
    let all_docs = db.get_all_documents(collection_name).unwrap();
    let doc_ids: Vec<String> = all_docs.iter()
        .map(|doc| doc.id.clone())
        .collect();
    
    let doc_ids_clone = doc_ids.clone();
    let start = Instant::now();
    let mut read_tasks = Vec::with_capacity(op_count);
    
    for i in 0..op_count {
        let db_clone = db.clone();
        let collection_name = collection_name.to_string();
        let doc_id = doc_ids_clone[i % doc_ids_clone.len()].clone();
        
        let task = task::spawn(async move {
            db_clone.get_document(&collection_name, &doc_id)
        });
        
        read_tasks.push(task);
    }
    
    future::join_all(read_tasks).await;
    let read_duration = start.elapsed();
    
    println!("Completed {} reads in {:?} ({} ops/sec)", 
        op_count, 
        read_duration,
        (op_count as f64 / read_duration.as_secs_f64()) as u64
    );
    
    // 3. Test concurrent updates
    println!("Testing {} concurrent updates...", op_count);
    
    let doc_ids_clone = doc_ids.clone();
    let start = Instant::now();
    let mut update_tasks = Vec::with_capacity(op_count);
    
    for i in 0..op_count {
        let db_clone = db.clone();
        let collection_name = collection_name.to_string();
        let doc_id = doc_ids_clone[i % doc_ids_clone.len()].clone();
        
        let task = task::spawn(async move {
            db_clone.update_document(
                &collection_name, 
                &doc_id, 
                json!({
                    "updated": true,
                    "update_count": i,
                    "updated_at": chrono::Utc::now().timestamp()
                })
            ).await
        });
        
        update_tasks.push(task);
    }
    
    future::join_all(update_tasks).await;
    let update_duration = start.elapsed();
    
    println!("Completed {} updates in {:?} ({} ops/sec)", 
        op_count, 
        update_duration,
        (op_count as f64 / update_duration.as_secs_f64()) as u64
    );
    
    // 4. Test concurrent queries
    println!("Testing {} concurrent queries...", op_count);
    
    let start = Instant::now();
    let mut query_tasks = Vec::with_capacity(op_count);
    
    for i in 0..op_count {
        let db_clone = db.clone();
        let collection_name = collection_name.to_string();
        
        // Create different query patterns to test different indexes
        let value_filter = i % 100;
        
        let task = task::spawn(async move {
            let query = Query {
                conditions: vec![
                    QueryCondition {
                        field: "value".to_string(),
                        operator: QueryOperator::Eq,
                        value: json!(value_filter),
                    }
                ],
                limit: Some(10),
                skip: None,
                sort_by: None,
            };
            
            db_clone.query_documents(&collection_name, query).await
        });
        
        query_tasks.push(task);
    }
    
    future::join_all(query_tasks).await;
    let query_duration = start.elapsed();
    
    println!("Completed {} queries in {:?} ({} ops/sec)", 
        op_count, 
        query_duration,
        (op_count as f64 / query_duration.as_secs_f64()) as u64
    );
    
    // 5. Test mixed workload (25% of each operation type)
    println!("Testing {} mixed operations...", op_count);
    
    let doc_ids_clone = doc_ids.clone();
    let start = Instant::now();
    let mut insert_tasks = Vec::new();
    let mut read_tasks = Vec::new();
    let mut update_tasks = Vec::new();
    let mut query_tasks = Vec::new();
    
    for i in 0..op_count {
        let db_clone = db.clone();
        let collection_name = collection_name.to_string();
        let operation_type = i % 4; // 0=insert, 1=read, 2=update, 3=query
        
        match operation_type {
            0 => {
                // Insert
                let task = task::spawn(async move {
                    let doc = Document::new(json!({
                        "mixed_index": i,
                        "name": format!("Mixed Doc {}", i),
                        "value": i % 100
                    }));
                    
                    db_clone.insert_document(&collection_name, doc).await
                });
                insert_tasks.push(task);
            },
            1 => {
                // Read
                let doc_id = doc_ids_clone[i % doc_ids_clone.len()].clone();
                let task = task::spawn(async move {
                    db_clone.get_document(&collection_name, &doc_id)
                });
                read_tasks.push(task);
            },
            2 => {
                // Update
                let doc_id = doc_ids_clone[i % doc_ids_clone.len()].clone();
                let task = task::spawn(async move {
                    db_clone.update_document(
                        &collection_name, 
                        &doc_id, 
                        json!({
                            "mixed_updated": true,
                            "update_index": i
                        })
                    ).await
                });
                update_tasks.push(task);
            },
            3 => {
                // Query
                let value_filter = i % 100;
                let task = task::spawn(async move {
                    let query = Query {
                        conditions: vec![
                            QueryCondition {
                                field: "value".to_string(),
                                operator: QueryOperator::Eq,
                                value: json!(value_filter),
                            }
                        ],
                        limit: Some(10),
                        skip: None,
                        sort_by: None,
                    };
                    
                    db_clone.query_documents(&collection_name, query).await
                });
                query_tasks.push(task);
            },
            _ => unreachable!(),
        }
    }
    
    // Wait for all tasks to complete
    let insert_results = future::join_all(insert_tasks).await;
    let read_results = future::join_all(read_tasks).await;
    let update_results = future::join_all(update_tasks).await;
    let query_results = future::join_all(query_tasks).await;
    let mixed_duration = start.elapsed();
    
    println!("Completed {} mixed operations in {:?} ({} ops/sec)", 
        op_count, 
        mixed_duration,
        (op_count as f64 / mixed_duration.as_secs_f64()) as u64
    );
    
    // Print a summary
    println!("\nPerformance Summary:");
    println!("--------------------");
    println!("Insert throughput: {} ops/sec", (op_count as f64 / insert_duration.as_secs_f64()) as u64);
    println!("Read throughput: {} ops/sec", (op_count as f64 / read_duration.as_secs_f64()) as u64);
    println!("Update throughput: {} ops/sec", (op_count as f64 / update_duration.as_secs_f64()) as u64);
    println!("Query throughput: {} ops/sec", (op_count as f64 / query_duration.as_secs_f64()) as u64);
    println!("Mixed throughput: {} ops/sec", (op_count as f64 / mixed_duration.as_secs_f64()) as u64);
    
    cleanup_temp_data_dir(&data_dir);
} 