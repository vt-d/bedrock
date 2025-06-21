#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod config;
pub mod discord;
pub mod nats;
pub mod runner;

use tracing::{info, span, Level};
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

    let nats_client = nats::connect(&config.nats_url).await?;
    nats::setup_jetstream(&nats_client).await?;

    let shards = discord::create_shards(&config);

    info!("Spawning shard runners");
    for shard in shards {
        let nats_client_clone = nats_client.clone();
        tokio::spawn(async move {
            runner::runner(shard, nats_client_clone).await;
        });
    }

    info!("System ready");

    tokio::signal::ctrl_c().await?;
    info!("Shutting down");

    Ok(())
}