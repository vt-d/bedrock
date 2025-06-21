use anyhow::Result;
use async_nats;
use tracing::{error, info, span, Level};

pub async fn connect(url: &str) -> Result<async_nats::Client> {
    info!(url = %url, "Connecting to NATS");
    let client = async_nats::connect(url).await?;
    info!("Connected to NATS successfully");
    Ok(client)
}

pub async fn setup_jetstream(client: &async_nats::Client) -> Result<()> {
    let nats_setup_span = span!(Level::INFO, "nats_setup");
    let _enter_nats = nats_setup_span.enter();

    let jetstream = async_nats::jetstream::new(client.clone());

    info!("ensuring 'discord-events' stream exists");
    match jetstream
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: "discord-events".to_string(),
            subjects: vec!["discord.shards.>".to_string()],
            max_messages: 10000,
            ..Default::default()
        })
        .await
    {
        Ok(_) => info!(stream.name = "discord-events", "ensured jetstream stream exists"),
        Err(e) => {
            error!(stream.name = "discord-events", error = %e, "failed to get or create jetstream stream");
            return Err(e.into());
        }
    }

    client
        .publish("discord.gateway.startup", "Bot is starting up!".into())
        .await?;

    info!("Published startup message");
    Ok(())
}
