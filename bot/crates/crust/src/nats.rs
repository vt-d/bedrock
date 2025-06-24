use crate::error::{CrustError, Result};
use crate::types::ShardGroup;
use async_nats;
use backoff::{ExponentialBackoff, Error as BackoffError, future::retry};
use chrono::Utc;
use tracing::{error, info};

pub async fn connect(url: &str) -> Result<async_nats::Client> {
    let operation = || async {
        info!(url = %url, "Connecting to NATS");
        async_nats::connect(url).await.map_err(|e| {
            error!(error = %e, "Failed to connect to NATS, retrying...");
            BackoffError::transient(e)
        })
    };

    let backoff = ExponentialBackoff::default();
    match retry(backoff, operation).await {
        Ok(client) => {
            info!("Connected to NATS successfully");
            Ok(client)
        }
        Err(e) => {
            error!(error = %e, "Failed to connect to NATS after multiple retries");
            Err(CrustError::Other(format!("Failed to connect to NATS: {}", e)))
        }
    }
}

pub async fn send_reshard_signal(
    nats_client: &async_nats::Client,
    new_shard_count: u32,
) -> Result<()> {
    let message = serde_json::json!({
        "event": "reshard",
        "new_shard_count": new_shard_count,
        "timestamp": Utc::now().to_rfc3339()
    });

    let operation = || async {
        nats_client
            .publish("discord.operator.reshard", message.to_string().into())
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to send reshard signal, retrying...");
                BackoffError::transient(e)
            })
    };

    let backoff = ExponentialBackoff::default();
    match retry(backoff, operation).await {
        Ok(_) => {
            info!(new_shard_count, "Sent reshard signal via NATS");
            Ok(())
        }
        Err(e) => {
            error!(error = %e, "Failed to send reshard signal after retries");
            Err(CrustError::Other(format!("Failed to send reshard signal: {}", e)))
        }
    }
}

pub async fn publish_startup_coordination(
    nats_client: &async_nats::Client, 
    cluster_name: &str,
    max_concurrency: u32,
    total_shards: u32,
    shard_groups: &[ShardGroup]
) -> Result<()> {
    let message = serde_json::json!({
        "event": "startup_coordination",
        "cluster": cluster_name,
        "max_concurrency": max_concurrency,
        "total_shards": total_shards,
        "shard_groups": shard_groups,
        "timestamp": Utc::now().to_rfc3339()
    });

    let operation = || async {
        nats_client
            .publish("discord.operator.startup", message.to_string().into())
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to send startup coordination, retrying...");
                BackoffError::transient(e)
            })
    };

    let backoff = ExponentialBackoff::default();
    match retry(backoff, operation).await {
        Ok(_) => {
            info!(
                cluster = %cluster_name,
                max_concurrency,
                total_shards,
                "Sent startup coordination via NATS"
            );
            Ok(())
        }
        Err(e) => {
            error!(error = %e, "Failed to send startup coordination after retries");
            Err(CrustError::Other(format!("Failed to send startup coordination: {}", e)))
        }
    }
}
