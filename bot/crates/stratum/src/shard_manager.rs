use crate::{config::Config, coordination::CoordinationHandler, discord, runner};
use async_nats::Client as NatsClient;
use std::collections::{HashMap, HashSet};
use tokio::task::JoinHandle;
use tracing::{error, info};

/// Manages Discord shards for this worker instance.
/// 
/// The ShardManager is responsible for:
/// - Starting and stopping Discord shards assigned to this worker
/// - Coordinating shard startup timing to respect Discord rate limits
/// - Handling dynamic shard count changes (resharding)
/// - Managing concurrency limits for shard connections
/// - Providing coordination interfaces for the operator
/// 
/// Each shard runs in its own async task, processing Discord events and
/// forwarding them to NATS. The manager ensures proper cleanup and restart
/// behavior when shards encounter errors or need to be reconfigured.
pub struct ShardManager {
    /// Configuration for this worker instance
    config: Config,
    /// NATS client for event publishing and coordination
    nats_client: NatsClient,
    /// Handler for coordination messages with the operator
    coordination: CoordinationHandler,
    /// Map of active shard tasks by shard ID
    shard_handles: HashMap<u32, JoinHandle<()>>,
    /// Shared Discord gateway configuration for all shards
    gateway_config: std::sync::Arc<twilight_gateway::Config>,
    /// Semaphore to limit concurrent shard connections
    startup_semaphore: std::sync::Arc<tokio::sync::Semaphore>,
}

impl ShardManager {
    /// Creates a new ShardManager instance.
    /// 
    /// This initializes all the components needed for shard management including
    /// the Discord gateway configuration, startup semaphore for concurrency control,
    /// and coordination handler for operator communication.
    /// 
    /// # Arguments
    /// 
    /// * `config` - The worker configuration containing shard assignments and limits
    /// * `nats_client` - NATS client for event publishing and coordination messages
    /// 
    /// # Returns
    /// 
    /// * `Ok(ShardManager)` - Successfully created shard manager
    /// * `Err(anyhow::Error)` - If Discord gateway configuration creation fails
    pub fn new(config: Config, nats_client: NatsClient) -> anyhow::Result<Self> {
        let gateway_config = discord::new_shard_manager_config(&config)?.gateway_config;
        
        let startup_semaphore = std::sync::Arc::new(
            tokio::sync::Semaphore::new(config.max_concurrency as usize)
        );
        
        let coordination = CoordinationHandler::new(nats_client.clone());
        
        Ok(Self {
            config,
            nats_client,
            coordination,
            shard_handles: HashMap::new(),
            gateway_config,
            startup_semaphore,
        })
    }

    /// Returns the worker ID for this instance.
    /// 
    /// # Returns
    /// 
    /// The unique worker identifier from the configuration.
    pub fn worker_id(&self) -> &str {
        &self.config.worker_id
    }

    /// Calculates the startup delay for this worker to stagger shard connections.
    /// 
    /// This implements a simple delay strategy based on the worker group number
    /// extracted from the worker ID. Each group waits an additional 10 seconds
    /// to spread out connection attempts across the Discord rate limit windows.
    /// 
    /// # Returns
    /// 
    /// Duration to wait before starting shards (group_number * 10 seconds).
    fn calculate_startup_delay(&self) -> std::time::Duration {
        let group_number = self.config.worker_id
            .strip_prefix("stratum-group-")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        
        std::time::Duration::from_secs(group_number as u64 * 10)
    }

    /// Starts all shards assigned to this worker.
    /// 
    /// This method:
    /// 1. Calculates the appropriate startup delay for rate limiting
    /// 2. Starts each assigned shard with a 2-second interval between connections
    /// 3. Logs startup progress and configuration
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - If all shards were started successfully
    /// * `Err(anyhow::Error)` - If shard configuration creation fails
    pub async fn start_shards(&mut self) -> anyhow::Result<()> {
        let shard_manager_config = discord::new_shard_manager_config(&self.config)?;
        
        let startup_delay = self.calculate_startup_delay();
        
        info!(
            "Starting shards: {:?}, with startup delay: {:?}",
            shard_manager_config.shard_ids,
            startup_delay
        );
        
        if startup_delay > std::time::Duration::ZERO {
            info!(
                worker_id = %self.config.worker_id,
                delay_seconds = startup_delay.as_secs(),
                "Waiting before starting shards to respect global concurrency"
            );
            tokio::time::sleep(startup_delay).await;
        }
        
        for shard_id_u32 in shard_manager_config.shard_ids {
            self.start_shard(shard_id_u32).await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        
        Ok(())
    }

    /// Starts a single shard in its own async task.
    /// 
    /// This method creates and spawns a task that:
    /// 1. Requests startup permission from the operator
    /// 2. Acquires a permit from the concurrency semaphore
    /// 3. Creates and runs the Discord shard
    /// 4. Notifies the operator when startup is complete
    /// 5. Automatically restarts on failure with a 5-second delay
    /// 
    /// The task runs in an infinite loop, ensuring the shard automatically
    /// restarts if it encounters errors or disconnections.
    /// 
    /// # Arguments
    /// 
    /// * `shard_id_u32` - The Discord shard ID to start
    async fn start_shard(&mut self, shard_id_u32: u32) {
        if self.shard_handles.contains_key(&shard_id_u32) {
            info!(shard_id = shard_id_u32, worker_id = %self.config.worker_id, "Shard already running, skipping");
            return;
        }

        let nats_client_clone = self.nats_client.clone();
        let gateway_config_clone = self.gateway_config.clone();
        let total_shards = self.config.total_shards;
        let worker_id = self.config.worker_id.clone();
        let startup_semaphore = self.startup_semaphore.clone();
        let coordination = CoordinationHandler::new(nats_client_clone.clone());

        let handle = tokio::spawn(async move {
            let shard_id = twilight_model::gateway::ShardId::new(shard_id_u32, total_shards);
            
            loop {
                if let Err(e) = coordination.request_startup_permission(&worker_id, shard_id_u32).await {
                    error!(worker_id = %worker_id, shard_id = shard_id.number(), error = ?e, "Failed to request startup permission");
                }
                
                let _permit = startup_semaphore.acquire().await.expect("Semaphore closed");
                
                info!(shard_id = shard_id.number(), worker_id = %worker_id, "Acquired startup permit, starting runner");
                
                let shard = twilight_gateway::Shard::with_config(shard_id, (*gateway_config_clone).clone());
                let nats_client_for_runner = nats_client_clone.clone();

                let result = runner::runner(shard, nats_client_for_runner).await;
                
                if let Err(e) = coordination.notify_startup_complete(&worker_id, shard_id_u32).await {
                    error!(worker_id = %worker_id, shard_id = shard_id.number(), error = ?e, "Failed to notify startup complete");
                }

                if let Err(e) = result {
                    error!(shard_id = shard_id.number(), worker_id = %worker_id, error = ?e, "Runner failed, restarting");
                    
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        });

        self.shard_handles.insert(shard_id_u32, handle);
        info!(shard_id = shard_id_u32, worker_id = %self.config.worker_id, "Started shard runner");
    }

    /// Stops a single shard by aborting its task.
    /// 
    /// This method removes the shard from the active handles map and
    /// aborts its async task, causing it to stop processing events.
    /// 
    /// # Arguments
    /// 
    /// * `shard_id_u32` - The Discord shard ID to stop
    async fn stop_shard(&mut self, shard_id_u32: u32) {
        if let Some(handle) = self.shard_handles.remove(&shard_id_u32) {
            handle.abort();
            info!(shard_id = shard_id_u32, worker_id = %self.config.worker_id, "Stopped shard runner");
        }
    }

    /// Updates the shard configuration when total shard count changes.
    /// 
    /// This method handles dynamic resharding by:
    /// 1. Updating the total shard count in the configuration
    /// 2. Calculating which shards this worker should now handle
    /// 3. Stopping shards that are no longer assigned to this worker
    /// 4. Starting new shards that are now assigned to this worker
    /// 
    /// # Arguments
    /// 
    /// * `new_total_shards` - The new total number of shards across the cluster
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - If shard update completed successfully
    /// * `Err(anyhow::Error)` - If shard configuration calculation fails
    pub async fn update_shards(&mut self, new_total_shards: u32) -> anyhow::Result<()> {
        info!(
            current_shards = self.config.total_shards,
            new_shards = new_total_shards,
            "Updating shard configuration"
        );

        self.config.total_shards = new_total_shards;
        
        let new_shard_manager_config = discord::new_shard_manager_config(&self.config)?;
        let new_shard_ids: HashSet<u32> = new_shard_manager_config.shard_ids.into_iter().collect();
        let current_shard_ids: HashSet<u32> = self.shard_handles.keys().cloned().collect();

        for shard_id in current_shard_ids.difference(&new_shard_ids) {
            self.stop_shard(*shard_id).await;
        }

        for shard_id in new_shard_ids.difference(&current_shard_ids) {
            self.start_shard(*shard_id).await;
        }

        info!(
            active_shards = ?new_shard_ids,
            "Shard update complete"
        );

        Ok(())
    }

    /// Shuts down all shards gracefully.
    /// 
    /// This method aborts all running shard tasks and clears the handles map.
    /// It's typically called during application shutdown to ensure clean termination.
    pub async fn shutdown(&mut self) {
        info!("Shutting down all shard runners");
        for (shard_id, handle) in self.shard_handles.drain() {
            handle.abort();
            info!(shard_id, "Stopped shard runner");
        }
    }

    /// Returns a reference to the coordination handler.
    /// 
    /// This allows external code to access the coordination functionality
    /// for listening to operator messages and sending coordination signals.
    /// 
    /// # Returns
    /// 
    /// A reference to the `CoordinationHandler` instance.
    pub fn coordination(&self) -> &CoordinationHandler {
        &self.coordination
    }
}
