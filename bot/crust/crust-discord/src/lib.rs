use crust_types::{CrustError, Result};
use twilight_http::Client as DiscordClient;
use tracing::info;

pub async fn get_gateway_info(client: &DiscordClient) -> Result<(u32, u32)> {
    let info = client
        .gateway()
        .authed()
        .await
        .map_err(|e| CrustError::Other(format!("Failed to get gateway info: {}", e)))?
        .model()
        .await
        .map_err(|e| CrustError::Other(format!("Failed to deserialize gateway info: {}", e)))?;
    
    info!(
        shards = info.shards,
        max_concurrency = info.session_start_limit.max_concurrency,
        "Retrieved Discord gateway information"
    );
    
    Ok((info.shards, info.session_start_limit.max_concurrency as u32))
}
