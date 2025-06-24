use crate::types::{Context, ShardCluster};
use chrono::Utc;
use kube::{
    api::{Api, ListParams, Patch, PatchParams},
    ResourceExt,
};
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info};

pub async fn reshard_scheduler(ctx: Context) {
    let mut interval = interval(Duration::from_secs(3600));

    loop {
        interval.tick().await;
        
        info!("Checking for clusters that need resharding");
        
        let shard_clusters: Api<ShardCluster> = Api::all(ctx.client.clone());
        
        match shard_clusters.list(&ListParams::default()).await {
            Ok(clusters) => {
                for cluster in clusters.items {
                    if should_reshard(&cluster) {
                        info!(cluster = %cluster.name_any(), "Triggering reshard");
                        
                        let patch = serde_json::json!({
                            "metadata": {
                                "annotations": {
                                    "crust.bedrock.dev/reshard-trigger": Utc::now().to_rfc3339()
                                }
                            }
                        });
                        
                        if let Err(e) = shard_clusters
                            .patch(
                                &cluster.name_any(),
                                &PatchParams::default(),
                                &Patch::Merge(&patch),
                            )
                            .await
                        {
                            error!(cluster = %cluster.name_any(), error = %e, "Failed to trigger reshard");
                        }
                    }
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to list ShardClusters");
            }
        }
    }
}

fn should_reshard(cluster: &ShardCluster) -> bool {
    if let Some(status) = &cluster.status {
        if let Some(last_reshard) = status.last_reshard {
            let reshard_interval = Duration::from_secs(cluster.spec.reshard_interval_hours * 3600);
            let time_since_reshard = Utc::now() - last_reshard;
            
            return time_since_reshard.to_std().unwrap_or(Duration::ZERO) >= reshard_interval;
        }
    }
    
    true
}
