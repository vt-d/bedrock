use async_nats::Client as NatsClient;
use futures_util::StreamExt;
use tracing::{error, info};

pub struct CoordinationHandler {
    nats_client: NatsClient,
}

impl CoordinationHandler {
    pub fn new(nats_client: NatsClient) -> Self {
        Self { nats_client }
    }

    pub async fn listen_for_reshard_signals(
        &self,
        shard_manager: std::sync::Arc<tokio::sync::RwLock<crate::shard_manager::ShardManager>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting reshard signal listener");
        
        let mut subscriber = self.nats_client.subscribe("discord.operator.reshard").await?;
        
        while let Some(message) = subscriber.next().await {
            info!(payload = %String::from_utf8_lossy(&message.payload), "Received reshard signal");
            
            if let Ok(reshard_data) = serde_json::from_slice::<serde_json::Value>(&message.payload) {
                if let Some(event) = reshard_data.get("event").and_then(|v| v.as_str()) {
                    if event == "reshard" {
                        if let Some(new_shard_count) = reshard_data.get("new_shard_count").and_then(|v| v.as_u64()) {
                            let manager = shard_manager.read().await;
                            let worker_id = manager.worker_id();
                            info!(new_shard_count, worker_id = %worker_id, "Processing reshard signal");
                            drop(manager);
                            
                            // Update shards dynamically
                            let mut manager = shard_manager.write().await;
                            if let Err(e) = manager.update_shards(new_shard_count as u32).await {
                                error!(error = ?e, worker_id = %manager.worker_id(), "Failed to update shards");
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    pub async fn listen_for_startup_coordination(
        &self,
        shard_manager: std::sync::Arc<tokio::sync::RwLock<crate::shard_manager::ShardManager>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting startup coordination listener");
        
        let mut subscriber = self.nats_client.subscribe("discord.operator.startup").await?;
        
        while let Some(message) = subscriber.next().await {
            info!(payload = %String::from_utf8_lossy(&message.payload), "Received startup coordination");
            
            if let Ok(startup_data) = serde_json::from_slice::<serde_json::Value>(&message.payload) {
                if let Some(event) = startup_data.get("event").and_then(|v| v.as_str()) {
                    if event == "startup_coordination" {
                        let manager = shard_manager.read().await;
                        let worker_id = manager.worker_id();
                        info!(worker_id = %worker_id, "Processing startup coordination signal");
                    }
                }
            }
        }
        
        Ok(())
    }

    pub async fn request_startup_permission(
        &self,
        worker_id: &str,
        shard_id: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = serde_json::json!({
            "action": "request_startup",
            "worker_id": worker_id,
            "shard_id": shard_id,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        self.nats_client
            .publish("discord.startup.request", request.to_string().into())
            .await?;
        
        info!(worker_id = %worker_id, shard_id, "Requested startup permission");
        Ok(())
    }

    pub async fn notify_startup_complete(
        &self,
        worker_id: &str,
        shard_id: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let notification = serde_json::json!({
            "action": "startup_complete",
            "worker_id": worker_id,
            "shard_id": shard_id,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        self.nats_client
            .publish("discord.startup.complete", notification.to_string().into())
            .await?;
        
        info!(worker_id = %worker_id, shard_id, "Notified startup complete");
        Ok(())
    }
}
