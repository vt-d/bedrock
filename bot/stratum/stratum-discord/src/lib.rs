use stratum_config::Config;
use anyhow::Result;
use std::sync::Arc;
use twilight_gateway::{Config as GatewayConfig, ConfigBuilder as GatewayConfigBuilder};
use twilight_model::gateway::Intents;

pub struct ShardManagerConfig {
    pub gateway_config: Arc<GatewayConfig>,
    pub shard_ids: std::ops::Range<u32>,
}

pub fn new_shard_manager_config(config: &Config) -> Result<ShardManagerConfig> {
    let gateway_config = Arc::new(
        GatewayConfigBuilder::new(config.discord_token.clone(), Intents::GUILD_MESSAGES).build(),
    );

    let shard_ids = config.shard_id_start..config.shard_id_end + 1;

    Ok(ShardManagerConfig {
        gateway_config,
        shard_ids,
    })
}
