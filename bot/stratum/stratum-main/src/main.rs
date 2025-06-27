#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;
use stratum_shard_manager::ShardManager;
use stratum_coordination::ShardManagerInterface;
use tokio::sync::RwLock;
use tracing::{error, info, span, Level};
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging()?;
    
    let config = stratum_config::Config::from_env()?;
    info!("Worker ID: {}", config.worker_id);

    let nats_client = connect_to_nats(&config.nats_url).await?;
    
    setup_jetstream(&nats_client).await?;
    run_application(config, nats_client).await
}

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

async fn connect_to_nats(nats_url: &str) -> anyhow::Result<async_nats::Client> {
    loop {
        match stratum_nats::connect(nats_url).await {
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

async fn setup_jetstream(nats_client: &async_nats::Client) -> anyhow::Result<()> {
    loop {
        match stratum_nats::setup_jetstream(nats_client).await {
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

async fn run_application(config: stratum_config::Config, nats_client: async_nats::Client) -> anyhow::Result<()> {
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

async fn shutdown(shard_manager: Arc<RwLock<ShardManager>>) {
    info!("Shutting down gracefully");
    
    let mut manager = shard_manager.write().await;
    manager.shutdown().await;
}
