use crate::config::Config;
use anyhow::Result;
use std::sync::Arc;
use twilight_gateway::{Config as GatewayConfig, ConfigBuilder as GatewayConfigBuilder};
use twilight_model::gateway::Intents;

/// Configuration required to spawn and manage Discord shards.
/// 
/// This struct contains the gateway configuration and shard ID range that
/// a shard manager instance needs to operate. It's designed to be created
/// once and shared across multiple shard-spawning tasks.
pub struct ShardManagerConfig {
    /// The base configuration for creating new shards.
    ///
    /// This is wrapped in an `Arc` to allow it to be shared across
    /// multiple shard-spawning tasks without cloning the entire configuration.
    pub gateway_config: Arc<GatewayConfig>,
    
    /// The range of shard IDs that this manager is responsible for.
    /// 
    /// This range determines which Discord shards this worker instance
    /// will connect to and manage. The range is inclusive of the end value.
    pub shard_ids: std::ops::Range<u32>,
}

/// Creates the configuration required for the shard manager.
///
/// This function takes the application configuration and creates a Discord
/// gateway configuration along with determining the shard ID range for this
/// worker instance. The gateway is configured with guild message intents
/// to receive Discord events.
/// 
/// # Arguments
/// 
/// * `config` - The application configuration containing Discord token and shard assignments
/// 
/// # Returns
/// 
/// * `Ok(ShardManagerConfig)` - Successfully created shard manager configuration
/// * `Err(anyhow::Error)` - If configuration creation fails
/// 
/// # Examples
/// 
/// ```no_run
/// use stratum::{config::Config, discord::new_shard_manager_config};
/// 
/// let config = Config::from_env().unwrap();
/// let shard_config = new_shard_manager_config(&config).unwrap();
/// ```
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
