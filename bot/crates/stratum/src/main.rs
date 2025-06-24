#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod config;
pub mod coordination;
pub mod discord;
pub mod nats;
pub mod runner;
pub mod shard_manager;

use std::sync::Arc;
use shard_manager::ShardManager;
use tokio::sync::RwLock;
use tracing::{error, info, span, Level};
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    init_logging()?;
    
    // Load configuration
    let config = config::Config::from_env()?;
    info!("Worker ID: {}", config.worker_id);

    // Connect to NATS with retry logic
    let nats_client = connect_to_nats(&config.nats_url).await?;
    
    // Setup JetStream with retry logic
    setup_jetstream(&nats_client).await?;

    // Run the main application
    run_application(config, nats_client).await
}

/// Initializes logging for the application
fn init_logging() -> anyhow::Result<()> {
    let subscriber = EnvFilter::from_default_env()
        .add_directive(Level::INFO.into())
        .add_directive("stratum=trace".parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(subscriber)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    Ok(())
}

/// Establishes connection to NATS with retry logic
async fn connect_to_nats(nats_url: &str) -> anyhow::Result<async_nats::Client> {
    loop {
        match nats::connect(nats_url).await {
            Ok(client) => {
                info!("Connected to NATS");
                return Ok(client);
            }
            Err(e) => {
                error!(error = ?e, "Failed to connect to NATS, retrying in 5 seconds");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

/// Sets up JetStream with retry logic
async fn setup_jetstream(nats_client: &async_nats::Client) -> anyhow::Result<()> {
    loop {
        match nats::setup_jetstream(nats_client).await {
            Ok(_) => {
                info!("JetStream setup complete");
                return Ok(());
            }
            Err(e) => {
                error!(error = ?e, "Failed to setup JetStream, retrying in 5 seconds");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

/// Runs the main application loop
async fn run_application(config: config::Config, nats_client: async_nats::Client) -> anyhow::Result<()> {
    let main_span = span!(Level::INFO, "main");
    let _enter = main_span.enter();

    info!("Starting application");

    let shard_manager = Arc::new(RwLock::new(
        ShardManager::new(config, nats_client)?
    ));

    // Start shards
    {
        let mut manager = shard_manager.write().await;
        info!("Starting shard manager for worker: {}", manager.worker_id());
        manager.start_shards().await?;
    }

    // Start coordination listeners
    let (reshard_handle, startup_handle) = start_coordination_listeners(&shard_manager).await;

    info!("System ready");

    // Wait for shutdown signal or listener failure
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        _ = reshard_handle => {
            info!("Reshard listener ended");
        }
        _ = startup_handle => {
            info!("Startup coordination listener ended");
        }
    }

    // Graceful shutdown
    shutdown(shard_manager).await;

    Ok(())
}

/// Starts the coordination listeners for reshard and startup signals
async fn start_coordination_listeners(
    shard_manager: &Arc<RwLock<ShardManager>>,
) -> (tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>) {
    let shard_manager_clone = shard_manager.clone();
    let reshard_handle = tokio::spawn(async move {
        let manager = shard_manager_clone.read().await;
        let coordination = manager.coordination();
        if let Err(e) = coordination.listen_for_reshard_signals(shard_manager_clone.clone()).await {
            error!(error = ?e, "Reshard listener failed");
        }
    });

    let shard_manager_clone2 = shard_manager.clone();
    let startup_handle = tokio::spawn(async move {
        let manager = shard_manager_clone2.read().await;
        let coordination = manager.coordination();
        if let Err(e) = coordination.listen_for_startup_coordination(shard_manager_clone2.clone()).await {
            error!(error = ?e, "Startup coordination listener failed");
        }
    });

    (reshard_handle, startup_handle)
}

/// Performs graceful shutdown of the application
async fn shutdown(shard_manager: Arc<RwLock<ShardManager>>) {
    info!("Shutting down gracefully");
    
    let mut manager = shard_manager.write().await;
    manager.shutdown().await;
}