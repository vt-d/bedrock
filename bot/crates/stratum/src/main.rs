#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod config;
pub mod discord;
pub mod nats;
pub mod runner;

use tracing::{error, info, span, Level};
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = EnvFilter::from_default_env()
        .add_directive(Level::INFO.into())
        .add_directive("stratum=trace".parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(subscriber)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let main_span = span!(Level::INFO, "main");
    let _enter = main_span.enter();

    info!("Starting application");

    let config = config::Config::from_env()?;

    let nats_client = loop {
        match nats::connect(&config.nats_url).await {
            Ok(client) => {
                info!("Connected to NATS");
                break client;
            }
            Err(e) => {
                error!(error = ?e, "Failed to connect to NATS, retrying in 5 seconds");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    };

    loop {
        match nats::setup_jetstream(&nats_client).await {
            Ok(_) => {
                info!("JetStream setup complete");
                break;
            }
            Err(e) => {
                error!(error = ?e, "Failed to setup JetStream, retrying in 5 seconds");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }

    let shard_manager_config = discord::new_shard_manager_config(&config)?;

    info!("Spawning shard runners");
    for shard_id_u32 in shard_manager_config.shard_ids {
        let nats_client_clone = nats_client.clone();
        let gateway_config_clone = shard_manager_config.gateway_config.clone();
        let total_shards = config.total_shards;

        tokio::spawn(async move {
            let shard_id =
                twilight_model::gateway::ShardId::new(shard_id_u32, total_shards);
            loop {
                let shard = twilight_gateway::Shard::with_config(
                    shard_id,
                    (*gateway_config_clone).clone(),
                );
                let nats_client_clone = nats_client_clone.clone();
                info!(shard_id = shard_id.number(), "Starting runner");

                let result = runner::runner(shard, nats_client_clone).await;

                if let Err(e) = result {
                    error!(shard_id = shard_id.number(), error = ?e, "Runner failed, restarting");
                }
            }
        });
    }

    info!("System ready");

    tokio::signal::ctrl_c().await?;
    info!("Shutting down");

    Ok(())
}