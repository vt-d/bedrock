use anyhow::Result;
use crust::{controller, nats, scheduler, types::{Context, ShardCluster}};
use futures::StreamExt;
use kube::{
    api::Api,
    runtime::{controller::Controller, watcher::Config},
    Client,
};
use std::sync::Arc;
use tracing::{debug, info, warn, Level};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = EnvFilter::from_default_env()
        .add_directive(Level::INFO.into())
        .add_directive("crust=debug".parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(subscriber)
        .init();

    info!("Starting Crust Kubernetes Operator");

    let client = Client::try_default().await?;
    
    let nats_url = std::env::var("NATS_URL")
        .unwrap_or_else(|_| "nats://localhost:4222".to_string());
    
    let nats_client = nats::connect(&nats_url).await?;
    
    let context = Context {
        client: client.clone(),
        nats_client,
    };

    let shard_clusters: Api<ShardCluster> = Api::all(client.clone());
    
    let controller = Controller::new(shard_clusters.clone(), Config::default())
        .run(controller::reconcile, controller::error_policy, Arc::new(context.clone()))
        .for_each(|res| async move {
            match res {
                Ok(o) => debug!("Reconciled {}", o.0.name),
                Err(e) => warn!("Reconcile failed: {}", e),
            }
        });

    let reshard_context = context.clone();
    let reshard_task = tokio::spawn(async move {
        scheduler::reshard_scheduler(reshard_context).await;
    });

    tokio::select! {
        _ = controller => warn!("Controller stream ended"),
        _ = reshard_task => warn!("Reshard scheduler ended"),
        _ = tokio::signal::ctrl_c() => info!("Received shutdown signal"),
    }

    info!("Shutting down operator");
    Ok(())
}
