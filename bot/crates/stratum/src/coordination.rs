use async_nats::Client as NatsClient;
use futures_util::StreamExt;
use tracing::{error, info};

/// Handles NATS-based coordination messages for the Discord bot.
/// 
/// This handler is responsible for managing communication between the Discord bot instances
/// and the Kubernetes operator, including reshard operations, startup coordination, and
/// permission requests. It uses NATS as the messaging backbone to ensure reliable
/// coordination across distributed bot instances.
pub struct CoordinationHandler {
    /// NATS client for publishing and subscribing to coordination messages
    nats_client: NatsClient,
}

impl CoordinationHandler {
    /// Creates a new coordination handler with the given NATS client.
    /// 
    /// # Arguments
    /// 
    /// * `nats_client` - The NATS client to use for messaging
    /// 
    /// # Returns
    /// 
    /// A new `CoordinationHandler` instance
    pub fn new(nats_client: NatsClient) -> Self {
        Self { nats_client }
    }

    /// Listens for reshard signals from the Kubernetes operator.
    /// 
    /// This function subscribes to the `discord.operator.reshard` NATS subject and
    /// processes reshard events. When a reshard event is received, it extracts the
    /// new shard count and triggers a dynamic shard update on the shard manager.
    /// 
    /// # Arguments
    /// 
    /// * `shard_manager` - Arc-wrapped RwLock of the ShardManager to update when resharding
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - This function runs indefinitely until an error occurs
    /// * `Err(Box<dyn std::error::Error>)` - If NATS subscription fails or message processing errors
    /// 
    /// # Message Format
    /// 
    /// Expected JSON message format:
    /// ```json
    /// {
    ///   "event": "reshard",
    ///   "new_shard_count": 4
    /// }
    /// ```
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

    /// Listens for startup coordination signals from the Kubernetes operator.
    /// 
    /// This function subscribes to the `discord.operator.startup` NATS subject and
    /// processes startup coordination events. Currently, it logs the received signals
    /// but can be extended to implement more sophisticated startup coordination logic.
    /// 
    /// # Arguments
    /// 
    /// * `shard_manager` - Arc-wrapped RwLock of the ShardManager for coordination
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - This function runs indefinitely until an error occurs
    /// * `Err(Box<dyn std::error::Error>)` - If NATS subscription fails or message processing errors
    /// 
    /// # Message Format
    /// 
    /// Expected JSON message format:
    /// ```json
    /// {
    ///   "event": "startup_coordination"
    /// }
    /// ```
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

    /// Requests permission from the operator to start a specific shard.
    /// 
    /// This function publishes a startup permission request to the `discord.startup.request`
    /// NATS subject. The operator can use this information to coordinate shard startups
    /// across multiple bot instances to respect rate limits and avoid conflicts.
    /// 
    /// # Arguments
    /// 
    /// * `worker_id` - The unique identifier of the worker requesting permission
    /// * `shard_id` - The Discord shard ID that the worker wants to start
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - If the request was successfully published to NATS
    /// * `Err(Box<dyn std::error::Error>)` - If NATS publishing fails
    /// 
    /// # Message Format
    /// 
    /// Publishes JSON message in the format:
    /// ```json
    /// {
    ///   "action": "request_startup",
    ///   "worker_id": "stratum-group-0",
    ///   "shard_id": 0,
    ///   "timestamp": 1640995200
    /// }
    /// ```
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

    /// Notifies the operator that a shard has completed its startup process.
    /// 
    /// This function publishes a startup completion notification to the 
    /// `discord.startup.complete` NATS subject. The operator can use this information
    /// to track the startup progress of shards across the cluster and coordinate
    /// subsequent startup operations.
    /// 
    /// # Arguments
    /// 
    /// * `worker_id` - The unique identifier of the worker that completed startup
    /// * `shard_id` - The Discord shard ID that has completed startup
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - If the notification was successfully published to NATS
    /// * `Err(Box<dyn std::error::Error>)` - If NATS publishing fails
    /// 
    /// # Message Format
    /// 
    /// Publishes JSON message in the format:
    /// ```json
    /// {
    ///   "action": "startup_complete",
    ///   "worker_id": "stratum-group-0", 
    ///   "shard_id": 0,
    ///   "timestamp": 1640995200
    /// }
    /// ```
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
