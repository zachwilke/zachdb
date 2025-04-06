use std::net::SocketAddr;
use std::sync::Arc;
use std::path::PathBuf;

use anyhow::{Result, Context};
use clap::Parser;
use tokio::signal;
use tracing::{info, warn, error};
use tracing_subscriber::EnvFilter;

mod core;
mod storage;
mod indexing;
mod api;
mod config;
mod utils;

use crate::config::Config;
use crate::core::Database;
use crate::api::{create_api_router, AppState};

/// Setup the tracing subscriber
fn setup_logging(log_level: &str) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("zachdb={}", log_level)));
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
    
    Ok(())
}

/// Run the database server
async fn run_server(config: Config) -> Result<()> {
    // Initialize the database
    info!("Initializing database '{}'...", config.name);
    let db = Database::new(config.name.clone(), config.data_dir.clone())
        .await
        .expect("Failed to initialize database");
    
    let db = Arc::new(db);
    
    // Create the API router
    let app_state = Arc::new(AppState { db: db.clone() });
    let app = create_api_router(app_state);
    
    // Bind to the configured address
    let addr = format!("{}:{}", config.host, config.port).parse::<SocketAddr>()?;
    
    info!("ZachDB server starting on {}", addr);
    info!("Database: {}", config.name);
    info!("Data directory: {}", config.data_dir.display());
    
    // Start the HTTP server
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("Error running server")?;
    
    // Perform cleanup
    info!("Shutting down database...");
    db.shutdown().await?;
    info!("Database shutdown complete");
    
    Ok(())
}

/// Graceful shutdown handler
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { info!("Received Ctrl+C, starting graceful shutdown..."); },
        _ = terminate => { info!("Received terminate signal, starting graceful shutdown..."); },
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let config = Config::parse();
    
    // Setup logging
    setup_logging(&config.log_level)?;
    
    info!("Starting ZachDB v{}", env!("CARGO_PKG_VERSION"));
    
    // Run the server
    if let Err(e) = run_server(config).await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
    
    Ok(())
}
