use std::path::PathBuf;
use std::sync::Arc;

use zachdb::core::Database;
use zachdb::api::{create_api_router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup logging
    tracing_subscriber::fmt()
        .with_env_filter("zachdb=info")
        .init();
    
    // Create data directory
    let data_dir = PathBuf::from("./zachdb-data");
    std::fs::create_dir_all(&data_dir)?;
    
    // Create the database
    let db = Database::new("simple-example".to_string(), data_dir)
        .await?;
    
    let db = Arc::new(db);
    
    // Create API router
    let app_state = Arc::new(AppState { db: db.clone() });
    let app = create_api_router(app_state);
    
    // Start the server
    let addr = "127.0.0.1:7878".parse()?;
    println!("ZachDB server starting on http://{}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
} 