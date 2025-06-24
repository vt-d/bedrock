use crate::{config::Config, coordination::CoordinationHandler, discord, runner};
use async_nats::Client as NatsClient;
use std::collections::{HashMap, HashSet};
use tokio::task::JoinHandle;
use tracing::{error, info};

pub struct ShardManager {
    config: Config,
    nats_client: NatsClient,
    coordination: CoordinationHandler,
    shard_handles: HashMap<u32, JoinHandle<()>>,
    gateway_config: std::sync::Arc<twilight_gateway::Config>,
    startup_semaphore: std::sync::Arc<tokio::sync::Semaphore>,
}

impl ShardManager {
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

    pub fn worker_id(&self) -> &str {
        &self.config.worker_id
    }

    fn calculate_startup_delay(&self) -> std::time::Duration {
        let group_number = self.config.worker_id
            .strip_prefix("stratum-group-")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        
        std::time::Duration::from_secs(group_number as u64 * 10)
    }

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

    async fn stop_shard(&mut self, shard_id_u32: u32) {
        if let Some(handle) = self.shard_handles.remove(&shard_id_u32) {
            handle.abort();
            info!(shard_id = shard_id_u32, worker_id = %self.config.worker_id, "Stopped shard runner");
        }
    }

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

    pub async fn shutdown(&mut self) {
        info!("Shutting down all shard runners");
        for (shard_id, handle) in self.shard_handles.drain() {
            handle.abort();
            info!(shard_id, "Stopped shard runner");
        }
    }

    pub fn coordination(&self) -> &CoordinationHandler {
        &self.coordination
    }
}
