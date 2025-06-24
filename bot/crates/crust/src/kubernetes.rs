use crate::error::{CrustError, Result};
use crate::types::{ShardCluster, ShardGroup};
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{Container, ContainerPort, EnvVar, PodSpec, PodTemplateSpec, Secret};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use kube::{
    api::{Api, ListParams, Patch, PatchParams, PostParams},
    Client, ResourceExt,
};
use std::collections::BTreeMap;
use tracing::info;

pub async fn get_discord_token(
    client: &Client,
    namespace: &str,
    secret_name: &str,
) -> Result<String> {
    let secrets: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let secret = secrets.get(secret_name).await?;
    
    let data = secret
        .data
        .ok_or_else(|| CrustError::Other("Secret has no data".to_string()))?;
    let token_bytes = data
        .get("token")
        .ok_or_else(|| CrustError::Other("Secret missing 'token' key".to_string()))?;
    
    String::from_utf8(token_bytes.0.clone())
        .map_err(|e| CrustError::Other(format!("Invalid UTF-8 in token: {}", e)))
}

pub fn calculate_shard_groups(total_shards: u32, shards_per_replica: u32) -> Vec<ShardGroup> {
    let mut groups = Vec::new();
    let mut current_shard = 0;
    let mut group_index = 0;

    while current_shard < total_shards {
        let shard_end = std::cmp::min(current_shard + shards_per_replica - 1, total_shards - 1);
        
        groups.push(ShardGroup {
            deployment_name: format!("stratum-group-{}", group_index),
            shard_start: current_shard,
            shard_end,
            replicas: 1,
        });

        current_shard = shard_end + 1;
        group_index += 1;
    }

    groups
}

pub async fn create_or_update_deployments(
    client: &Client,
    namespace: &str,
    cluster: &ShardCluster,
    shard_groups: &[ShardGroup],
    total_shards: u32,
    max_concurrency: u32,
) -> Result<()> {
    let deployments: Api<Deployment> = Api::namespaced(client.clone(), namespace);
    
    let list_params = ListParams::default().labels(&format!(
        "managed-by=crust-operator,app=stratum,cluster={}",
        cluster.name_any()
    ));
    
    let existing_deployments = deployments.list(&list_params).await?;
    let existing_names: std::collections::HashSet<String> = existing_deployments
        .items
        .iter()
        .filter_map(|d| d.metadata.name.clone())
        .collect();
    
    let new_names: std::collections::HashSet<String> = shard_groups
        .iter()
        .map(|g| g.deployment_name.clone())
        .collect();
    
    for group in shard_groups {
        let deployment = create_deployment_spec(cluster, group, namespace, total_shards, max_concurrency)?;
        
        match deployments.get(&group.deployment_name).await {
            Ok(_) => {
                deployments
                    .patch(
                        &group.deployment_name,
                        &PatchParams::default(),
                        &Patch::Merge(&deployment),
                    )
                    .await?;
                info!(deployment = %group.deployment_name, "Updated deployment");
            }
            Err(_) => {
                deployments
                    .create(&PostParams::default(), &deployment)
                    .await?;
                info!(deployment = %group.deployment_name, "Created deployment");
            }
        }
    }
    
    for old_deployment in existing_names.difference(&new_names) {
        deployments
            .delete(old_deployment, &Default::default())
            .await?;
        info!(deployment = %old_deployment, "Deleted unnecessary deployment");
    }
    
    Ok(())
}

fn create_deployment_spec(
    cluster: &ShardCluster,
    group: &ShardGroup,
    namespace: &str,
    total_shards: u32,
    max_concurrency: u32,
) -> Result<Deployment> {
    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), "stratum".to_string());
    labels.insert("shard-group".to_string(), group.deployment_name.clone());
    labels.insert("managed-by".to_string(), "crust-operator".to_string());
    labels.insert("cluster".to_string(), cluster.name_any());

    let env_vars = vec![
        EnvVar {
            name: "NATS_URL".to_string(),
            value: Some(cluster.spec.nats_url.clone()),
            value_from: None,
        },
        EnvVar {
            name: "SHARD_ID_START".to_string(),
            value: Some(group.shard_start.to_string()),
            value_from: None,
        },
        EnvVar {
            name: "SHARD_ID_END".to_string(),
            value: Some(group.shard_end.to_string()),
            value_from: None,
        },
        EnvVar {
            name: "TOTAL_SHARDS".to_string(),
            value: Some(total_shards.to_string()),
            value_from: None,
        },
        EnvVar {
            name: "WORKER_ID".to_string(),
            value: Some(group.deployment_name.clone()),
            value_from: None,
        },
        EnvVar {
            name: "MAX_CONCURRENCY".to_string(),
            value: Some(max_concurrency.to_string()),
            value_from: None,
        },
        EnvVar {
            name: "DISCORD_TOKEN".to_string(),
            value: None,
            value_from: Some(k8s_openapi::api::core::v1::EnvVarSource {
                secret_key_ref: Some(k8s_openapi::api::core::v1::SecretKeySelector {
                    name: cluster.spec.discord_token_secret.clone(),
                    key: "token".to_string(),
                    optional: None,
                }),
                ..Default::default()
            }),
        },
    ];

    let deployment = Deployment {
        metadata: ObjectMeta {
            name: Some(group.deployment_name.clone()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(group.replicas),
            selector: LabelSelector {
                match_labels: Some(labels.clone()),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "stratum".to_string(),
                        image: Some(cluster.spec.image.clone()),
                        image_pull_policy: Some("Never".to_string()),
                        env: Some(env_vars),
                        ports: Some(vec![ContainerPort {
                            container_port: 8080,
                            name: Some("metrics".to_string()),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    Ok(deployment)
}
