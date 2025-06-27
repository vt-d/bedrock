use chrono::{DateTime, Utc};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[kube(group = "bedrock.dev", version = "v1", kind = "ShardCluster")]
#[kube(status = "ShardClusterStatus")]
#[kube(shortname = "sc")]
#[kube(namespaced)]
pub struct ShardClusterSpec {
    pub discord_token_secret: String,
    pub nats_url: String,
    pub image: String,
    pub replicas_per_shard_group: i32,
    pub shards_per_replica: u32,
    pub reshard_interval_hours: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ShardClusterStatus {
    pub current_shards: Option<u32>,
    #[schemars(with = "Option<String>")]
    pub last_reshard: Option<DateTime<Utc>>,
    pub shard_groups: Vec<ShardGroup>,
    pub phase: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ShardGroup {
    pub deployment_name: String,
    pub shard_start: u32,
    pub shard_end: u32,
    pub replicas: i32,
}

#[derive(Clone)]
pub struct Context {
    pub client: kube::Client,
    pub nats_client: async_nats::Client,
}
