use anyhow::Result;
use tracing::info;

/// Configuration for the Discord bot worker instance.
/// 
/// This struct holds all the configuration parameters needed for a Discord bot worker
/// to operate within a distributed cluster. It handles shard assignment, NATS connectivity,
/// Discord API authentication, and concurrency limits.
#[derive(Clone)]
pub struct Config {
    /// NATS server URL for messaging and coordination
    pub nats_url: String,
    /// Discord bot token for API authentication
    pub discord_token: String,
    /// Starting shard ID for this worker (inclusive)
    pub shard_id_start: u32,
    /// Ending shard ID for this worker (inclusive)
    pub shard_id_end: u32,
    /// Total number of shards across the entire cluster
    pub total_shards: u32,
    /// Unique identifier for this worker instance
    pub worker_id: String,
    /// Maximum number of concurrent shard connections
    pub max_concurrency: u32,
}

impl Config {
    /// Creates a new configuration instance from environment variables.
    /// 
    /// This method reads configuration from the following environment variables:
    /// - `NATS_URL`: NATS server URL (default: "nats://localhost:4222")
    /// - `DISCORD_TOKEN`: Discord bot token (required)
    /// - `SHARD_ID_START`: Starting shard ID for this worker (required)
    /// - `SHARD_ID_END`: Ending shard ID for this worker (required)
    /// - `TOTAL_SHARDS`: Total shards across the cluster (required)
    /// - `WORKER_ID`: Unique worker identifier (default: "unknown")
    /// - `MAX_CONCURRENCY`: Max concurrent shard connections (default: "1")
    /// 
    /// # Returns
    /// 
    /// * `Ok(Config)` - Successfully parsed configuration
    /// * `Err(anyhow::Error)` - If required variables are missing or parsing fails
    /// 
    /// # Panics
    /// 
    /// Panics if required environment variables (`DISCORD_TOKEN`, `SHARD_ID_START`, 
    /// `SHARD_ID_END`, `TOTAL_SHARDS`) are not set.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use stratum::config::Config;
    /// 
    /// // Set required environment variables
    /// std::env::set_var("DISCORD_TOKEN", "your_bot_token");
    /// std::env::set_var("SHARD_ID_START", "0");
    /// std::env::set_var("SHARD_ID_END", "1");
    /// std::env::set_var("TOTAL_SHARDS", "4");
    /// 
    /// let config = Config::from_env().unwrap();
    /// ```
    pub fn from_env() -> Result<Self> {
        let nats_url =
            std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
        let discord_token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");
        let shard_id_start: u32 = std::env::var("SHARD_ID_START")
            .expect("SHARD_ID_START must be set")
            .parse()?;
        let shard_id_end: u32 = std::env::var("SHARD_ID_END")
            .expect("SHARD_ID_END must be set")
            .parse()?;
        let total_shards: u32 = std::env::var("TOTAL_SHARDS")
            .expect("TOTAL_SHARDS must be set")
            .parse()?;
        let worker_id = std::env::var("WORKER_ID")
            .unwrap_or_else(|_| "unknown".to_string());
        let max_concurrency: u32 = std::env::var("MAX_CONCURRENCY")
            .unwrap_or_else(|_| "1".to_string())
            .parse()?;

        info!(
            shard_id_start,
            shard_id_end, 
            total_shards, 
            worker_id = %worker_id,
            max_concurrency,
            "Loaded cluster configuration"
        );

        Ok(Self {
            nats_url,
            discord_token,
            shard_id_start,
            shard_id_end,
            total_shards,
            worker_id,
            max_concurrency,
        })
    }

    /// Returns the worker ID for this instance.
    /// 
    /// # Returns
    /// 
    /// A string slice containing the unique worker identifier.
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }
}
