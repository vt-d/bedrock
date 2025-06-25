use anyhow::Result;
use async_nats;
use backoff::{Error as BackoffError, ExponentialBackoff, future::retry};
use futures_util::StreamExt;
use tracing::{Level, error, info, span, trace};
use twilight_gateway::{Message, Shard, error::ReceiveMessageErrorType};

/// Runs a Discord shard and forwards events to NATS.
/// 
/// This function is the core event processing loop for a Discord shard. It:
/// 1. Publishes a startup message to NATS indicating the shard is starting
/// 2. Continuously processes Discord gateway events
/// 3. Forwards all text messages to NATS JetStream for consumption by other services
/// 4. Handles reconnection scenarios and errors gracefully
/// 
/// The runner publishes events to subject patterns:
/// - `discord.shards.{shard_id}.startup` - Shard startup notifications
/// - `discord.shards.{shard_id}.events` - All Discord gateway events
/// 
/// # Arguments
/// 
/// * `shard` - The Discord gateway shard to run
/// * `nats_client` - NATS client for publishing events
/// 
/// # Returns
/// 
/// * `Ok(())` - If the shard shuts down gracefully
/// * `Err(anyhow::Error)` - If an unrecoverable error occurs
/// 
/// # Error Handling
/// 
/// - **Reconnect errors**: Function returns to allow restart by caller
/// - **Publish errors**: Retried with exponential backoff
/// - **Other gateway errors**: Logged but processing continues
/// 
/// # Examples
/// 
/// ```no_run
/// use stratum::{runner::runner, nats::connect};
/// use twilight_gateway::Shard;
/// use twilight_model::gateway::ShardId;
/// 
/// let shard = Shard::new(ShardId::new(0, 1), "token".to_string(), Default::default());
/// let nats_client = connect("nats://localhost:4222").await.unwrap();
/// 
/// // This will run indefinitely until an error occurs
/// runner(shard, nats_client).await.unwrap();
/// ```
pub async fn runner(mut shard: Shard, nats_client: async_nats::Client) -> Result<()> {
    let runner_span = span!(
        Level::INFO,
        "discord_shard_runner",
        shard.id = shard.id().number()
    );
    let _enter = runner_span.enter();

    info!("Starting Discord shard runner");

    let subject = format!("discord.shards.{}.startup", shard.id().number());
    let startup_message = format!("Shard {} is starting", shard.id().number());

    let publish_op = || async {
        nats_client
            .publish(subject.clone(), startup_message.clone().into())
            .await
            .map_err(BackoffError::transient)
    };

    let backoff = ExponentialBackoff::default();
    retry(backoff, publish_op).await?;
    info!(
        shard.id = shard.id().number(),
        "Published shard startup message to NATS"
    );

    while let Some(event) = shard.next().await {
        let event_span = span!(Level::TRACE, "discord_event_handling");
        let _enter_event = event_span.enter();
        match event {
            Ok(message) => {
                let Some(bytes) = (match message {
                    Message::Text(text) => Some(text.into_bytes()),
                    Message::Close(_) => None,
                }) else {
                    continue;
                };

                let subject = format!("discord.shards.{}.events", shard.id().number());
                let publish_op = || async {
                    nats_client
                        .publish(subject.clone(), bytes.clone().into())
                        .await
                        .map_err(BackoffError::transient)
                };

                let backoff = ExponentialBackoff::default();
                retry(backoff, publish_op).await?;
                trace!(subject = %subject, "Published event to NATS");
            }
            Err(e) => {
                error!(error = %e, "Error processing event from Discord");
                match e.kind() {
                    ReceiveMessageErrorType::Reconnect => {
                        return Err(e.into());
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
