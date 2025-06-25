//! Stratum - Discord Bot Event Ingestion Layer
//! 
//! Stratum is the foundational data ingestion component of the Bedrock Discord bot platform.
//! It connects to Discord's real-time gateway, processes events from multiple shards, and
//! forwards them to NATS JetStream for consumption by other services.
//! 
//! ## Architecture
//! 
//! - **Shard Management**: Manages multiple Discord gateway connections (shards) with proper
//!   startup coordination and concurrency limits
//! - **Event Processing**: Receives Discord events and forwards them to NATS topics
//! - **Operator Coordination**: Communicates with the Kubernetes operator for resharding
//!   and startup coordination across the cluster
//! - **Resilience**: Automatic reconnection and error recovery for stable operation
//! 
//! ## Configuration
//! 
//! Stratum is configured via environment variables:
//! - `DISCORD_TOKEN`: Discord bot authentication token
//! - `NATS_URL`: NATS server connection string  
//! - `SHARD_ID_START`/`SHARD_ID_END`: Shard range for this worker
//! - `TOTAL_SHARDS`: Total shards across the cluster
//! - `WORKER_ID`: Unique identifier for this worker instance
//! - `MAX_CONCURRENCY`: Maximum concurrent shard connections
//! 
//! ## Event Flow
//! 
//! 1. Discord Gateway → Shard Runners → NATS JetStream
//! 2. Events published to `discord.shards.{shard_id}.events`
//! 3. Startup notifications on `discord.shards.{shard_id}.startup`
//! 4. Coordination via `discord.operator.*` topics

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
    init_logging()?;
    
    let config = config::Config::from_env()?;
    info!("Worker ID: {}", config.worker_id);

    let nats_client = connect_to_nats(&config.nats_url).await?;
    
    setup_jetstream(&nats_client).await?;
    run_application(config, nats_client).await
}

/// Initializes the tracing logging system.
/// 
/// Sets up structured logging with:
/// - INFO level for general application logs
/// - TRACE level for stratum-specific logs
/// - Span close events for timing information
/// 
/// # Returns
/// 
/// * `Ok(())` - If logging initialization succeeds
/// * `Err(anyhow::Error)` - If log filter parsing fails
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

/// Establishes connection to NATS with retry logic.
/// 
/// Continuously attempts to connect to the NATS server until successful.
/// Uses exponential backoff with 5-second retry intervals for failed attempts.
/// 
/// # Arguments
/// 
/// * `nats_url` - The NATS server URL to connect to
/// 
/// # Returns
/// 
/// * `Ok(async_nats::Client)` - Successfully connected NATS client
/// * `Err(anyhow::Error)` - This function retries indefinitely, so errors are rare
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

/// Sets up NATS JetStream for event processing.
/// 
/// Configures the JetStream environment and verifies connectivity by
/// creating the discord-events stream. Retries with 5-second intervals
/// until JetStream is ready and stream is created successfully.
/// 
/// # Arguments
/// 
/// * `nats_client` - Connected NATS client to use for JetStream setup
/// 
/// # Returns
/// 
/// * `Ok(())` - If JetStream setup completes successfully
/// * `Err(anyhow::Error)` - This function retries indefinitely, so errors are rare
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

/// Runs the main application logic.
/// 
/// This function orchestrates the core application flow:
/// 1. Creates and initializes the shard manager
/// 2. Starts all assigned Discord shards 
/// 3. Launches coordination listeners for operator communication
/// 4. Waits for shutdown signal or listener failure
/// 5. Performs graceful shutdown of all components
/// 
/// # Arguments
/// 
/// * `config` - Application configuration
/// * `nats_client` - Connected NATS client for event publishing
/// 
/// # Returns
/// 
/// * `Ok(())` - If application shuts down gracefully
/// * `Err(anyhow::Error)` - If critical errors occur during startup or operation
async fn run_application(config: config::Config, nats_client: async_nats::Client) -> anyhow::Result<()> {
    let main_span = span!(Level::INFO, "main");
    let _enter = main_span.enter();

    info!("Starting application");

    let shard_manager = Arc::new(RwLock::new(
        ShardManager::new(config, nats_client)?
    ));

    {
        let mut manager = shard_manager.write().await;
        info!("Starting shard manager for worker: {}", manager.worker_id());
        manager.start_shards().await?;
    }

    let (reshard_handle, startup_handle) = start_coordination_listeners(&shard_manager).await;

    info!("System ready");

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

    shutdown(shard_manager).await;

    Ok(())
}

/// Starts the coordination listeners for operator communication.
/// 
/// Spawns async tasks to listen for:
/// - Reshard signals from the operator on `discord.operator.reshard`
/// - Startup coordination messages on `discord.operator.startup`
/// 
/// Both listeners run indefinitely until an error occurs or the application shuts down.
/// 
/// # Arguments
/// 
/// * `shard_manager` - Shared shard manager for coordination operations
/// 
/// # Returns
/// 
/// A tuple of join handles for the reshard and startup coordination tasks.
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

/// Performs graceful shutdown of the application.
/// 
/// Shuts down the shard manager, which in turn stops all running Discord
/// shards and cleans up their associated resources. This ensures a clean
/// application termination without leaving orphaned connections.
/// 
/// # Arguments
/// 
/// * `shard_manager` - The shard manager to shut down
async fn shutdown(shard_manager: Arc<RwLock<ShardManager>>) {
    info!("Shutting down gracefully");
    
    let mut manager = shard_manager.write().await;
    manager.shutdown().await;
}