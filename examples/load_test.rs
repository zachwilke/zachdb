use std::sync::Arc;
use std::time::{Instant, Duration};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::collections::HashMap;

use reqwest::Client;
use tokio::task;
use tokio::time;
use futures::future;
use serde_json::json;
use uuid::Uuid;
use rand::Rng;
use std::sync::atomic::AtomicBool;
use chrono::Utc;
use anyhow::{Result, anyhow};

// Configuration for load test
struct LoadTestConfig {
    base_url: String,
    collection_name: String,
    num_clients: usize,
    operations_per_client: usize,
    test_duration_seconds: u64,
}

/// Statistics for load test
struct LoadTestStats {
    operation_count: AtomicUsize,
    success_count: AtomicUsize,
    error_count: AtomicUsize,
    
    // Per-operation statistics
    create_count: AtomicUsize,
    create_success: AtomicUsize,
    create_errors: AtomicUsize,
    create_latency: AtomicUsize,
    
    get_count: AtomicUsize,
    get_success: AtomicUsize,
    get_errors: AtomicUsize,
    get_latency: AtomicUsize,
    
    update_count: AtomicUsize,
    update_success: AtomicUsize,
    update_errors: AtomicUsize,
    update_latency: AtomicUsize,
    
    query_count: AtomicUsize,
    query_success: AtomicUsize,
    query_errors: AtomicUsize,
    query_latency: AtomicUsize,
}

impl LoadTestStats {
    fn new() -> Self {
        Self {
            operation_count: AtomicUsize::new(0),
            success_count: AtomicUsize::new(0),
            error_count: AtomicUsize::new(0),
            
            create_count: AtomicUsize::new(0),
            create_success: AtomicUsize::new(0),
            create_errors: AtomicUsize::new(0),
            create_latency: AtomicUsize::new(0),
            
            get_count: AtomicUsize::new(0),
            get_success: AtomicUsize::new(0),
            get_errors: AtomicUsize::new(0),
            get_latency: AtomicUsize::new(0),
            
            update_count: AtomicUsize::new(0),
            update_success: AtomicUsize::new(0),
            update_errors: AtomicUsize::new(0),
            update_latency: AtomicUsize::new(0),
            
            query_count: AtomicUsize::new(0),
            query_success: AtomicUsize::new(0),
            query_errors: AtomicUsize::new(0),
            query_latency: AtomicUsize::new(0),
        }
    }
    
    fn record_success(&self, op_type: &str, latency: Duration) {
        self.operation_count.fetch_add(1, Ordering::Relaxed);
        self.success_count.fetch_add(1, Ordering::Relaxed);
        
        let latency_ms = latency.as_millis() as usize;
        
        match op_type {
            "create" => {
                self.create_count.fetch_add(1, Ordering::Relaxed);
                self.create_success.fetch_add(1, Ordering::Relaxed);
                self.create_latency.fetch_add(latency_ms, Ordering::Relaxed);
            },
            "get" => {
                self.get_count.fetch_add(1, Ordering::Relaxed);
                self.get_success.fetch_add(1, Ordering::Relaxed);
                self.get_latency.fetch_add(latency_ms, Ordering::Relaxed);
            },
            "update" => {
                self.update_count.fetch_add(1, Ordering::Relaxed);
                self.update_success.fetch_add(1, Ordering::Relaxed);
                self.update_latency.fetch_add(latency_ms, Ordering::Relaxed);
            },
            "query" => {
                self.query_count.fetch_add(1, Ordering::Relaxed);
                self.query_success.fetch_add(1, Ordering::Relaxed);
                self.query_latency.fetch_add(latency_ms, Ordering::Relaxed);
            },
            _ => {},
        }
    }
    
    fn record_error(&self, op_type: &str) {
        self.operation_count.fetch_add(1, Ordering::Relaxed);
        self.error_count.fetch_add(1, Ordering::Relaxed);
        
        match op_type {
            "create" => {
                self.create_count.fetch_add(1, Ordering::Relaxed);
                self.create_errors.fetch_add(1, Ordering::Relaxed);
            },
            "get" => {
                self.get_count.fetch_add(1, Ordering::Relaxed);
                self.get_errors.fetch_add(1, Ordering::Relaxed);
            },
            "update" => {
                self.update_count.fetch_add(1, Ordering::Relaxed);
                self.update_errors.fetch_add(1, Ordering::Relaxed);
            },
            "query" => {
                self.query_count.fetch_add(1, Ordering::Relaxed);
                self.query_errors.fetch_add(1, Ordering::Relaxed);
            },
            _ => {},
        }
    }
    
    fn print_stats(&self, duration_secs: f64) {
        let total_ops = self.operation_count.load(Ordering::Relaxed);
        let success_ops = self.success_count.load(Ordering::Relaxed);
        let error_ops = self.error_count.load(Ordering::Relaxed);
        
        println!("\nLoad Test Results:");
        println!("------------------");
        println!("Total operations: {}", total_ops);
        println!("Successful operations: {} ({:.2}%)", 
            success_ops, 
            (success_ops as f64 / total_ops as f64) * 100.0
        );
        println!("Failed operations: {} ({:.2}%)", 
            error_ops,
            (error_ops as f64 / total_ops as f64) * 100.0
        );
        
        println!("Operations per second: {:.2}", total_ops as f64 / duration_secs);
        
        // Print per-operation stats
        println!("\nPer-Operation Results:");
        self.print_op_stats("Create", &self.create_count, &self.create_success, &self.create_errors, &self.create_latency, duration_secs);
        self.print_op_stats("Get", &self.get_count, &self.get_success, &self.get_errors, &self.get_latency, duration_secs);
        self.print_op_stats("Update", &self.update_count, &self.update_success, &self.update_errors, &self.update_latency, duration_secs);
        self.print_op_stats("Query", &self.query_count, &self.query_success, &self.query_errors, &self.query_latency, duration_secs);
    }
    
    fn print_op_stats(
        &self, 
        op_name: &str, 
        count: &AtomicUsize, 
        success: &AtomicUsize, 
        errors: &AtomicUsize,
        latency: &AtomicUsize,
        duration_secs: f64
    ) {
        let op_count = count.load(Ordering::Relaxed);
        if op_count == 0 {
            return;
        }
        
        let op_success = success.load(Ordering::Relaxed);
        let op_errors = errors.load(Ordering::Relaxed);
        let total_latency = latency.load(Ordering::Relaxed);
        
        println!("\n{}:", op_name);
        println!("  Count: {}", op_count);
        println!("  Success: {} ({:.2}%)", 
            op_success, 
            (op_success as f64 / op_count as f64) * 100.0
        );
        println!("  Errors: {} ({:.2}%)", 
            op_errors,
            (op_errors as f64 / op_count as f64) * 100.0
        );
        println!("  Avg Latency: {:.2} ms", 
            total_latency as f64 / op_success as f64
        );
        println!("  Throughput: {:.2} ops/sec", 
            op_count as f64 / duration_secs
        );
    }
}

/// Creates a collection
async fn create_collection(client: &Client, config: &Arc<LoadTestConfig>) -> Result<()> {
    let url = format!("{}/collections/{}", config.base_url, config.collection_name);
    
    let response = client.put(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Request failed: {}", e))?;
    
    if response.status().is_success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to create collection: {}", response.status()))
    }
}

/// Creates a document
async fn create_document(client: &Client, config: &Arc<LoadTestConfig>, data: HashMap<String, serde_json::Value>) -> Result<String> {
    let url = format!("{}/collections/{}/documents", config.base_url, config.collection_name);
    
    let response = client.post(&url)
        .json(&data)
        .send()
        .await
        .map_err(|e| anyhow!("Request failed: {}", e))?;
    
    if response.status().is_success() {
        let result: serde_json::Value = response.json().await
            .map_err(|e| anyhow!("Failed to parse response: {}", e))?;
        
        // Extract document ID
        match result.get("id") {
            Some(id) => Ok(id.as_str().unwrap_or_default().to_string()),
            None => Err(anyhow!("Response missing document ID")),
        }
    } else {
        Err(anyhow!("Failed to create document: {}", response.status()))
    }
}

/// Gets a document
async fn get_document(client: &Client, config: &Arc<LoadTestConfig>, doc_id: &str) -> Result<serde_json::Value> {
    let url = format!("{}/collections/{}/documents/{}", config.base_url, config.collection_name, doc_id);
    
    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Request failed: {}", e))?;
    
    if response.status().is_success() {
        response.json().await
            .map_err(|e| anyhow!("Failed to parse response: {}", e))
    } else {
        Err(anyhow!("Failed to get document: {}", response.status()))
    }
}

/// Updates a document
async fn update_document(client: &Client, config: &Arc<LoadTestConfig>, doc_id: &str, data: HashMap<String, serde_json::Value>) -> Result<serde_json::Value> {
    let url = format!("{}/collections/{}/documents/{}", config.base_url, config.collection_name, doc_id);
    
    let response = client.put(&url)
        .json(&data)
        .send()
        .await
        .map_err(|e| anyhow!("Request failed: {}", e))?;
    
    if response.status().is_success() {
        response.json().await
            .map_err(|e| anyhow!("Failed to parse response: {}", e))
    } else {
        Err(anyhow!("Failed to update document: {}", response.status()))
    }
}

/// Queries documents
async fn query_documents(client: &Client, config: &Arc<LoadTestConfig>, query: HashMap<String, serde_json::Value>) -> Result<Vec<serde_json::Value>> {
    let url = format!("{}/collections/{}/query", config.base_url, config.collection_name);
    
    let response = client.post(&url)
        .json(&query)
        .send()
        .await
        .map_err(|e| anyhow!("Request failed: {}", e))?;
    
    if response.status().is_success() {
        response.json().await
            .map_err(|e| anyhow!("Failed to parse response: {}", e))
    } else {
        Err(anyhow!("Failed to query documents: {}", response.status()))
    }
}

/// Simulates a client performing operations
async fn client_worker(
    client: Client,
    config: Arc<LoadTestConfig>,
    stats: Arc<LoadTestStats>,
    doc_ids: Arc<Mutex<Vec<String>>>,
    stop_signal: Arc<AtomicBool>,
) {
    // Using a static rng for thread safety
    let client_id = Uuid::new_v4();
    let _start = Instant::now();
    
    while !stop_signal.load(Ordering::Relaxed) {
        // Randomly choose an operation to perform
        let operation = {
            // Create a new RNG for each operation to avoid Send issues
            let mut rng = rand::thread_rng();
            let choice = rng.gen_range(0..=3);
            match choice {
                0 => "create",
                1 => "get",
                2 => "update",
                _ => "query",
            }
        };
        
        let op_start = Instant::now();
        
        match operation {
            "create" => {
                // Generate a random document
                let doc_id = Uuid::new_v4().to_string();
                let mut data = HashMap::new();
                
                // Add some random fields
                {
                    let mut rng = rand::thread_rng();
                    data.insert("value".to_string(), json!(rng.gen_range(1..1000)));
                    data.insert("name".to_string(), json!(format!("Test Document {}", doc_id)));
                    data.insert("active".to_string(), json!(rng.gen_bool(0.5)));
                    data.insert("created_by".to_string(), json!(client_id.to_string()));
                }
                
                match create_document(&client, &config, data).await {
                    Ok(id) => {
                        let mut ids = doc_ids.lock().unwrap();
                        ids.push(id);
                        stats.record_success("create", op_start.elapsed());
                    },
                    Err(_) => stats.record_error("create"),
                }
            },
            "get" => {
                // Get a random document ID
                let doc_id = {
                    let ids = doc_ids.lock().unwrap();
                    if ids.is_empty() {
                        continue; // Skip if no documents yet
                    }
                    
                    let mut rng = rand::thread_rng();
                    ids[rng.gen_range(0..ids.len())].clone()
                };
                
                match get_document(&client, &config, &doc_id).await {
                    Ok(_) => stats.record_success("get", op_start.elapsed()),
                    Err(_) => stats.record_error("get"),
                }
            },
            "update" => {
                // Get a random document ID
                let doc_id = {
                    let ids = doc_ids.lock().unwrap();
                    if ids.is_empty() {
                        continue; // Skip if no documents yet
                    }
                    
                    let mut rng = rand::thread_rng();
                    ids[rng.gen_range(0..ids.len())].clone()
                };
                
                // Generate update data
                let mut data = HashMap::new();
                {
                    let mut rng = rand::thread_rng();
                    data.insert("updated".to_string(), json!(true));
                    data.insert("value".to_string(), json!(rng.gen_range(1..1000)));
                    data.insert("updated_at".to_string(), json!(Utc::now().timestamp()));
                }
                
                match update_document(&client, &config, &doc_id, data).await {
                    Ok(_) => stats.record_success("update", op_start.elapsed()),
                    Err(_) => stats.record_error("update"),
                }
            },
            "query" => {
                // Generate a random query
                let mut query = HashMap::new();
                let mut conditions = Vec::new();
                
                {
                    let mut rng = rand::thread_rng();
                    let condition = serde_json::json!({
                        "field": "value",
                        "operator": "gt",
                        "value": rng.gen_range(1..500)
                    });
                    conditions.push(condition);
                }
                
                query.insert("conditions".to_string(), json!(conditions));
                query.insert("limit".to_string(), json!(10));
                
                match query_documents(&client, &config, query).await {
                    Ok(_) => stats.record_success("query", op_start.elapsed()),
                    Err(_) => stats.record_error("query"),
                }
            },
            _ => unreachable!(),
        }
        
        // Sleep a bit to simulate user think time
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure the load test
    let config = Arc::new(LoadTestConfig {
        base_url: "http://localhost:7878".to_string(),
        collection_name: format!("loadtest-{}", Uuid::new_v4()),
        num_clients: 10,
        operations_per_client: 1000,
        test_duration_seconds: 30,
    });
    
    println!("Starting load test with the following configuration:");
    println!("- Server URL: {}", config.base_url);
    println!("- Collection: {}", config.collection_name);
    println!("- Clients: {}", config.num_clients);
    println!("- Operations per client: {}", config.operations_per_client);
    println!("- Duration: {} seconds", config.test_duration_seconds);
    
    // Create a shared HTTP client
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    
    // Create the test collection
    create_collection(&client, &config).await?;
    println!("Created test collection: {}", config.collection_name);
    
    // Create shared statistics
    let stats = Arc::new(LoadTestStats::new());
    
    // Create a vector to store document IDs
    let doc_ids = Arc::new(Mutex::new(Vec::new()));
    
    // Create a stop signal
    let stop_signal = Arc::new(AtomicBool::new(false));
    
    // Spawn client workers
    let mut handles = Vec::with_capacity(config.num_clients);
    
    for _ in 0..config.num_clients {
        let client_clone = client.clone();
        let stats_clone = stats.clone();
        let doc_ids_clone = doc_ids.clone();
        let stop_signal_clone = stop_signal.clone();
        let config_clone = config.clone();
        
        let handle = task::spawn(client_worker(
            client_clone,
            config_clone,
            stats_clone,
            doc_ids_clone,
            stop_signal_clone,
        ));
        
        handles.push(handle);
    }
    
    // Run the test for the specified duration
    let start_time = Instant::now();
    let test_duration = Duration::from_secs(config.test_duration_seconds);
    
    // Wait for the test to complete
    time::sleep(test_duration).await;
    
    // Signal workers to stop
    stop_signal.store(true, Ordering::Relaxed);
    
    // Wait for all workers to finish
    future::join_all(handles).await;
    
    // Calculate the actual test duration
    let actual_duration = start_time.elapsed();
    
    // Print statistics
    stats.print_stats(actual_duration.as_secs_f64());
    
    Ok(())
} 