use crate::discord;
use crate::error::{CrustError, Result};
use crate::kubernetes;
use crate::nats;
use crate::types::{Context, ShardCluster, ShardClusterStatus};
use chrono::Utc;
use kube::{
    api::{Api, Patch, PatchParams},
    runtime::controller::Action,
    ResourceExt,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use twilight_http::Client as DiscordClient;

pub async fn reconcile(cluster: Arc<ShardCluster>, ctx: Arc<Context>) -> Result<Action> {
    let name = cluster.name_any();
    let namespace = cluster.namespace().unwrap_or_else(|| "default".to_string());
    
    info!(cluster = %name, namespace = %namespace, "Reconciling ShardCluster");

    if let Some(status) = &cluster.status {
        if let Some(last_reshard) = status.last_reshard {
            let time_since_last_update = Utc::now().signed_duration_since(last_reshard);
            if time_since_last_update.num_minutes() < 10 {
                info!(
                    cluster = %name,
                    minutes_since_update = time_since_last_update.num_minutes(),
                    "Recent update detected, skipping Discord API call"
                );
                return Ok(Action::requeue(Duration::from_secs(600))); // Requeue in 10 minutes
            }
        }
    }

    let discord_token = kubernetes::get_discord_token(
        &ctx.client,
        &namespace,
        &cluster.spec.discord_token_secret,
    ).await?;
    let discord_client = DiscordClient::new(discord_token);
    
    let (recommended_shards, max_concurrency) = discord::get_gateway_info(discord_client).await?;
    info!(
        cluster = %name, 
        recommended_shards, 
        max_concurrency,
        "Got Discord gateway info"
    );

    let shard_clusters: Api<ShardCluster> = Api::namespaced(ctx.client.clone(), &namespace);
    
    let new_shard_groups = kubernetes::calculate_shard_groups(
        recommended_shards,
        cluster.spec.shards_per_replica,
    );
    
    let current_shard_groups = cluster.status.as_ref()
        .map(|s| s.shard_groups.len())
        .unwrap_or(0);
    
    let needs_deployment_update = current_shard_groups != new_shard_groups.len();
    
    if needs_deployment_update {
        info!(
            cluster = %name,
            current_groups = current_shard_groups,
            new_groups = new_shard_groups.len(),
            "Shard group count changed, updating deployments"
        );
        
        kubernetes::create_or_update_deployments(
            &ctx.client,
            &namespace,
            &cluster,
            &new_shard_groups,
            recommended_shards,
            max_concurrency,
        ).await?;
    }
    
    nats::send_reshard_signal(&ctx.nats_client, recommended_shards).await?;
    
    nats::publish_startup_coordination(
        &ctx.nats_client,
        &name,
        max_concurrency,
        recommended_shards,
        &new_shard_groups
    ).await?;

    // Update status
    let status = ShardClusterStatus {
        current_shards: Some(recommended_shards),
        last_reshard: Some(Utc::now()),
        shard_groups: new_shard_groups,
        phase: "Active".to_string(),
    };

    let status_patch = serde_json::json!({
        "status": status
    });

    shard_clusters
        .patch_status(&name, &PatchParams::default(), &Patch::Merge(&status_patch))
        .await?;

    Ok(Action::requeue(Duration::from_secs(1800))) // Requeue every 30 minutes
}

pub fn error_policy(_object: Arc<ShardCluster>, error: &CrustError, _ctx: Arc<Context>) -> Action {
    error!(error = %error, "Reconciliation error");
    
    if error.to_string().contains("429") || error.to_string().contains("rate limit") {
        error!("Rate limit detected, backing off for 5 minutes");
        Action::requeue(Duration::from_secs(300)) // 5 minutes for rate limits
    } else {
        Action::requeue(Duration::from_secs(120)) // 2 minutes for other errors
    }
}
