use crate::config::Config;
use tracing::{info, span, Level};
use twilight_gateway::{Intents, Shard};

pub fn create_shards(config: &Config) -> Vec<Shard> {
    let discord_setup_span = span!(Level::INFO, "discord_setup");
    let _enter_discord = discord_setup_span.enter();

    let intents = Intents::GUILDS | Intents::GUILD_MESSAGES;

    let shard_ids = config.shard_id_start..config.shard_id_end;
    let gateway_config = twilight_gateway::Config::new(config.discord_token.clone(), intents);

    let shards: Vec<Shard> = twilight_gateway::create_iterator(
        shard_ids,
        config.total_shards,
        gateway_config,
        |_, builder| builder.build(),
    )
    .collect();

    info!("Discord shards created successfully");
    shards
}
