use async_nats;
use backoff::{future::retry, Error as BackoffError, ExponentialBackoff};
use futures_util::StreamExt;
use tracing::{error, info, span, trace, Level};
use twilight_gateway::{Message, Shard};

pub async fn runner(mut shard: Shard, nats_client: async_nats::Client) {
    let runner_span = span!(Level::INFO, "discord_shard_runner", shard.id = shard.id().number());
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
    if let Err(e) = retry(backoff, publish_op).await {
        error!(error = %e, "Failed to publish shard startup message to NATS after multiple retries");
    } else {
        info!(shard.id = shard.id().number(), "Published shard startup message to NATS");
    }

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
                if let Err(e) = retry(backoff, publish_op).await {
                    error!(error = %e, "Failed to publish event to NATS after multiple retries");
                } else {
                    trace!(subject = %subject, "Published event to NATS");
                }
            }
            Err(e) => {
                error!(error = %e, "Error processing event from Discord");
            }
        }
    }
}
