use anyhow::Result;
use async_nats;
use backoff::{Error as BackoffError, ExponentialBackoff, future::retry};
use tracing::{Level, error, info, span};

pub async fn connect(url: &str) -> Result<async_nats::Client> {
    let operation = || async {
        info!(url = %url, "Connecting to NATS");
        async_nats::connect(url).await.map_err(|e| {
            error!(error = %e, "Failed to connect to NATS, retrying...");
            BackoffError::transient(e)
        })
    };

    let backoff = ExponentialBackoff::default();
    match retry(backoff, operation).await {
        Ok(client) => {
            info!("Connected to NATS successfully");
            Ok(client)
        }
        Err(e) => {
            error!(error = %e, "Failed to connect to NATS after multiple retries");
            Err(e.into())
        }
    }
}

pub async fn setup_jetstream(client: &async_nats::Client) -> Result<()> {
    let nats_setup_span = span!(Level::INFO, "nats_setup");
    let _enter_nats = nats_setup_span.enter();

    let jetstream = async_nats::jetstream::new(client.clone());

    info!("ensuring 'discord-events' stream exists");

    // First, let's try to check if JetStream is available by creating the stream directly
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
                BackoffError::transient(e)
            })
    };

    let mut backoff = ExponentialBackoff::default();
    backoff.max_elapsed_time = Some(std::time::Duration::from_secs(300)); // 5 minutes
    if let Err(e) = retry(backoff, stream_op).await {
        error!(stream.name = "discord-events", error = %e, "failed to get or create jetstream stream after all retries");
        return Err(e.into());
    }
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
                BackoffError::transient(e)
            })
    };

    let backoff = ExponentialBackoff::default();
    if let Err(e) = retry(backoff, publish_op).await {
        error!(error = %e, "Failed to publish startup message after all retries");
        return Err(e.into());
    }

    info!("Published startup message");
    Ok(())
}
