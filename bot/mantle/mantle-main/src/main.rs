use futures::StreamExt;
use serde::de::DeserializeSeed;
use twilight_model::gateway::event::GatewayEventDeserializer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nats = async_nats::connect("nats://localhost:4222").await?;
    let jetstream = async_nats::jetstream::new(nats);
    
    let consumer = jetstream
        .create_consumer_on_stream(
            async_nats::jetstream::consumer::pull::Config {
                durable_name: Some("mantle-processors".to_string()),
                description: Some("Mantle event processors - work queue".to_string()),
                ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                max_deliver: 3,
                ..Default::default()
            },
            "discord-events",
        )
        .await?;

    println!("Mantle processor started, waiting for events...");

    let mut messages = consumer.messages().await?;
    while let Some(message) = messages.next().await {
        match message {
            Ok(msg) => {
                if let Err(e) = process_discord_event(&msg.payload).await {
                    eprintln!("Failed to process event: {}", e);
                    if let Err(ack_err) = msg.ack_with(async_nats::jetstream::AckKind::Nak(None)).await {
                        eprintln!("Failed to NAK message: {}", ack_err);
                    }
                } else {
                    if let Err(ack_err) = msg.ack().await {
                        eprintln!("Failed to ACK message: {}", ack_err);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error receiving message: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
    
    Ok(())
}

async fn process_discord_event(payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let payload_str = std::str::from_utf8(payload)?;
    let deserializer = GatewayEventDeserializer::from_json(payload_str)
        .ok_or("Failed to create deserializer")?;
    let mut json_deserializer = serde_json::Deserializer::from_str(payload_str);
    let event = deserializer.deserialize(&mut json_deserializer)?;
    
    println!("Processing event: {:?}", event);
    
    Ok(())
}
