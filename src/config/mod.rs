use std::path::PathBuf;
use clap::Parser;

/// Command line arguments for the database
#[derive(Debug, Parser)]
#[clap(name = "zachdb", about = "A lightweight, high-performance, non-relational database")]
pub struct Config {
    /// The name of the database
    #[clap(long, default_value = "zachdb")]
    pub name: String,
    
    /// The directory where database files are stored
    #[clap(long, default_value = "./zachdb-data")]
    pub data_dir: PathBuf,
    
    /// The host address to bind to
    #[clap(long, default_value = "127.0.0.1")]
    pub host: String,
    
    /// The port to bind to
    #[clap(long, default_value = "7878")]
    pub port: u16,
    
    /// Maximum number of connections
    #[clap(long, default_value = "100")]
    pub max_connections: usize,
    
    /// Enable HTTP logging
    #[clap(long)]
    pub log_http: bool,
    
    /// Log level (trace, debug, info, warn, error)
    #[clap(long, default_value = "info")]
    pub log_level: String,
} 