use anyhow::Result;
use tracing::info;

#[derive(Clone)]
pub struct Config {
    pub nats_url: String,
    pub discord_token: String,
    pub shard_id_start: u32,
    pub shard_id_end: u32,
    pub total_shards: u32,
    pub worker_id: String,
    pub max_concurrency: u32,
}

impl Config {
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

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }
}
