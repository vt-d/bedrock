use std::sync::LazyLock;
use std::time::Duration;

pub static CLIENT: LazyLock<twilight_http::Client> = LazyLock::new(|| {
    let proxy_url = std::env::var("TWILIGHT_PROXY_URL")
        .unwrap_or_else(|_| "http://twilight-gateway-proxy.bedrock.svc.cluster.local".to_string());
    
    twilight_http::Client::builder()
        .token(std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set"))
        .proxy(proxy_url, false)  // Production: Use HTTP proxy
        .timeout(Duration::from_secs(30))  // Production: 30 second timeout
        .build()
});