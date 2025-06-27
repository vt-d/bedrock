use anyhow::Result;
use async_nats;
use backon::{ExponentialBuilder, Retryable};
use futures_util::StreamExt;
use tracing::{Level, error, info, span, trace};
use twilight_gateway::{Message, Shard, error::ReceiveMessageErrorType};

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
    };

    let backoff = ExponentialBuilder::default().with_max_times(5);
    publish_op.retry(&backoff).await?;
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
                };

                let backoff = ExponentialBuilder::default().with_max_times(5);
                publish_op.retry(&backoff).await?;
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
