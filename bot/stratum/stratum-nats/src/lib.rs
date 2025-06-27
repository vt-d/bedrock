use anyhow::Result;
use async_nats;
use backon::{ExponentialBuilder, Retryable};
use tracing::{Level, error, info, span};

pub async fn connect(url: &str) -> Result<async_nats::Client> {
    let operation = || async {
        info!(url = %url, "Connecting to NATS");
        async_nats::connect(url).await.map_err(|e| {
            error!(error = %e, "Failed to connect to NATS, retrying...");
            e
        })
    };

    let backoff = ExponentialBuilder::default().with_max_times(10);
    let client = operation.retry(&backoff).await?;
    
    info!("Connected to NATS successfully");
    Ok(client)
}

pub async fn setup_jetstream(client: &async_nats::Client) -> Result<()> {
    let nats_setup_span = span!(Level::INFO, "nats_setup");
    let _enter_nats = nats_setup_span.enter();

    let jetstream = async_nats::jetstream::new(client.clone());

    info!("ensuring 'discord-events' stream exists");

    info!("Checking JetStream availability...");

    let stream_op = || async {
        jetstream
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: "discord-events".to_string(),
                subjects: vec!["discord.shards.>".to_string()],
                max_messages: 10000,
                ..Default::default()
            })
            .await
            .map_err(|e| {
                error!(stream.name = "discord-events", error = %e, "failed to get or create jetstream stream, retrying...");
                e
            })
    };

    let backoff = ExponentialBuilder::default()
        .with_max_times(20)
        .with_max_delay(std::time::Duration::from_secs(60));
    
    stream_op.retry(&backoff).await.map_err(|e| {
        error!(stream.name = "discord-events", error = %e, "failed to get or create jetstream stream after all retries");
        e
    })?;
    
    info!(
        stream.name = "discord-events",
        "ensured jetstream stream exists"
    );

    let publish_op = || async {
        client
            .publish("discord.gateway.startup", "Bot is starting up!".into())
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to publish startup message, retrying...");
                e
            })
    };

    let backoff = ExponentialBuilder::default().with_max_times(10);
    publish_op.retry(&backoff).await.map_err(|e| {
        error!(error = %e, "Failed to publish startup message after all retries");
        e
    })?;

    info!("Published startup message");
    Ok(())
}
