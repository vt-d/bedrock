use anyhow::Result;
use async_nats;
use backoff::{Error as BackoffError, ExponentialBackoff, future::retry};
use tracing::{Level, error, info, span};

/// Establishes a connection to the NATS server with retry logic.
/// 
/// This function attempts to connect to the specified NATS server URL with
/// exponential backoff retry logic. It will continue retrying until a successful
/// connection is established or the maximum retry attempts are reached.
/// 
/// # Arguments
/// 
/// * `url` - The NATS server URL to connect to (e.g., "nats://localhost:4222")
/// 
/// # Returns
/// 
/// * `Ok(async_nats::Client)` - Successfully connected NATS client
/// * `Err(anyhow::Error)` - If connection fails after all retry attempts
/// 
/// # Examples
/// 
/// ```no_run
/// use stratum::nats::connect;
/// 
/// let client = connect("nats://localhost:4222").await.unwrap();
/// ```
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

/// Sets up JetStream for Discord event processing.
/// 
/// This function configures the NATS JetStream environment for handling Discord events.
/// It creates the "discord-events" stream with appropriate configuration and publishes
/// a startup message to verify connectivity. The stream is configured to handle all
/// Discord shard events with a maximum message limit.
/// 
/// # Arguments
/// 
/// * `client` - The connected NATS client to use for JetStream operations
/// 
/// # Returns
/// 
/// * `Ok(())` - If JetStream setup completed successfully
/// * `Err(anyhow::Error)` - If stream creation or message publishing fails
/// 
/// # Stream Configuration
/// 
/// - **Name**: "discord-events"
/// - **Subjects**: "discord.shards.>" (all Discord shard events)
/// - **Max Messages**: 10,000
/// - **Retention**: Default (limits-based)
/// 
/// # Examples
/// 
/// ```no_run
/// use stratum::nats::{connect, setup_jetstream};
/// 
/// let client = connect("nats://localhost:4222").await.unwrap();
/// setup_jetstream(&client).await.unwrap();
/// ```
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
