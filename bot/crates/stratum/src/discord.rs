use crate::config::Config;
use anyhow::Result;
use std::sync::Arc;
use twilight_gateway::{Config as GatewayConfig, ConfigBuilder as GatewayConfigBuilder};
use twilight_model::gateway::Intents;

/// Configuration required to spawn and manage Discord shards.
pub struct ShardManagerConfig {
    /// The base configuration for creating new shards.
    ///
    /// This is wrapped in an `Arc` to allow it to be shared across
    /// multiple shard-spawning tasks.
    pub gateway_config: Arc<GatewayConfig>,
    /// The range of shard IDs that this manager is responsible for.
    pub shard_ids: std::ops::Range<u32>,
}

/// Creates the configuration required for the shard manager.
///
/// This involves creating a base `twilight_gateway` configuration and
/// determining the range of shard IDs to manage from the application's
/// overall configuration.
pub fn new_shard_manager_config(config: &Config) -> Result<ShardManagerConfig> {
    let gateway_config = Arc::new(
        GatewayConfigBuilder::new(config.discord_token.clone(), Intents::empty()).build(),
    );

    let shard_ids = config.shard_id_start..config.shard_id_end;

    Ok(ShardManagerConfig {
        gateway_config,
        shard_ids,
    })
}