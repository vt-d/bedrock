#!/bin/bash

# Bedrock Production Deployment Script
# This script deploys Bedrock with production-ready configurations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CLUSTER_NAME="bedrock-production"
NAMESPACE="bedrock"
NATS_NAMESPACE="nats-system"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Validate production readiness
validate_production_readiness() {
    print_status "Validating production readiness..."
    
    # Check if Discord token is set
    if [ -z "$DISCORD_TOKEN" ]; then
        print_error "DISCORD_TOKEN environment variable is required for production"
        exit 1
    fi
    
    # Check if using tagged images
    if grep -q ":latest" "$PROJECT_ROOT/k8s/operator-deployment.yaml"; then
        print_warning "Using :latest tags in production is not recommended"
    fi
    
    # Check resource limits
    if ! grep -q "limits:" "$PROJECT_ROOT/k8s/operator-deployment.yaml"; then
        print_error "Resource limits are required for production"
        exit 1
    fi
    
    print_success "Production validation passed"
}

# Deploy with production settings
deploy_production() {
    print_status "Deploying Bedrock in production mode..."
    
    # Create namespaces
    kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -
    kubectl create namespace "$NATS_NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -
    
    # Create secrets
    kubectl create secret generic discord-token \
        --from-literal=token="$DISCORD_TOKEN" \
        --namespace="$NAMESPACE" \
        --dry-run=client -o yaml | kubectl apply -f -
    
    # Deploy NATS with production settings
    print_status "Deploying NATS cluster (production configuration)..."
    kubectl apply -f "$PROJECT_ROOT/k8s/nats-cluster.yaml"
    
    # Wait for NATS to be ready
    print_status "Waiting for NATS cluster to be ready..."
    kubectl wait --for=condition=ready pod -l app=nats -n "$NATS_NAMESPACE" --timeout=300s
    
    # Deploy CRD
    print_status "Deploying ShardCluster CRD..."
    kubectl apply -f "$PROJECT_ROOT/crd/shardcluster-crd.yaml"
    
    # Deploy RBAC
    print_status "Deploying RBAC with leader election permissions..."
    kubectl apply -f "$PROJECT_ROOT/k8s/rbac.yaml"
    
    # Deploy Twilight Gateway Proxy
    print_status "Deploying Twilight Gateway Proxy (production configuration)..."
    kubectl apply -f "$PROJECT_ROOT/k8s/twilight-gateway-proxy.yaml"
    
    # Wait for proxy to be ready
    print_status "Waiting for proxy to be ready..."
    kubectl wait --for=condition=available deployment/twilight-gateway-proxy -n "$NAMESPACE" --timeout=300s
    
    # Deploy operator
    print_status "Deploying Bedrock operator with high availability..."
    kubectl apply -f "$PROJECT_ROOT/k8s/operator-deployment.yaml"
    
    # Wait for operator to be ready
    print_status "Waiting for operator to be ready..."
    kubectl wait --for=condition=available deployment/crust-operator -n "$NAMESPACE" --timeout=300s
    
    # Deploy production ShardCluster
    print_status "Deploying production Bedrock bot cluster..."
    deploy_shard_cluster
    
    print_success "Production deployment completed successfully!"
}

# Deploy production ShardCluster
deploy_shard_cluster() {
    cat <<EOF | kubectl apply -f -
apiVersion: bedrock.dev/v1
kind: ShardCluster
metadata:
  name: bedrock-discord-bot
  namespace: $NAMESPACE
  labels:
    environment: production
    app: bedrock-bot
spec:
  discord_token_secret: "discord-token"
  nats_url: "nats://nats-cluster.nats-system.svc.cluster.local:4222"
  image: "ghcr.io/vt-d/bedrock/stratum:sha-4530824"  # Production: Use tagged version
  replicas_per_shard_group: 2   # Production: 2 replicas per shard group for HA
  shards_per_replica: 8         # Production: More shards per replica for efficiency
  reshard_interval_hours: 12    # Production: More frequent resharding
EOF
    
    print_success "Production Bedrock bot cluster deployed"
}

# Show production status
show_production_status() {
    print_status "Production deployment status:"
    echo
    
    print_status "NATS Cluster (High Availability):"
    kubectl get pods -n "$NATS_NAMESPACE" -l app=nats -o wide
    echo
    
    print_status "Operator (Leader Election Enabled):"
    kubectl get pods -n "$NAMESPACE" -l app=crust-operator -o wide
    echo
    
    print_status "Gateway Proxy (Load Balanced):"
    kubectl get pods -n "$NAMESPACE" -l app=twilight-gateway-proxy -o wide
    echo
    
    print_status "Bot Instances (ShardCluster):"
    kubectl get shardclusters -n "$NAMESPACE" -o wide
    echo
    kubectl get pods -n "$NAMESPACE" -l app=stratum -o wide 2>/dev/null || print_warning "No bot pods found yet"
    echo
    
    print_status "Resource Usage:"
    kubectl top pods -n "$NAMESPACE" 2>/dev/null || print_warning "Metrics server not available"
    echo
    
    print_success "Production monitoring commands:"
    echo "# Monitor operator logs:"
    echo "kubectl logs -l app=crust-operator -n $NAMESPACE -f"
    echo
    echo "# Monitor proxy logs:"
    echo "kubectl logs -l app=twilight-gateway-proxy -n $NAMESPACE -f"
    echo
    echo "# Monitor bot instance logs:"
    echo "kubectl logs -l app=stratum -n $NAMESPACE -f"
    echo
    echo "# Monitor NATS logs:"
    echo "kubectl logs -l app=nats -n $NATS_NAMESPACE -f"
    echo
    echo "# Check ShardCluster status:"
    echo "kubectl get shardclusters -n $NAMESPACE -o yaml"
    echo
    echo "# Check cluster health:"
    echo "kubectl get all -n $NAMESPACE"
    echo "kubectl get all -n $NATS_NAMESPACE"
}

# Main execution
main() {
    echo "🚀 Bedrock Production Deployment"
    echo "================================"
    echo
    
    validate_production_readiness
    deploy_production
    
    # Wait for things to settle
    sleep 10
    show_production_status
}

# Handle interrupts
trap 'print_error "Production deployment interrupted"; exit 1' INT TERM

# Check if we have the required Discord token
if [ -z "$DISCORD_TOKEN" ]; then
    print_error "Please set DISCORD_TOKEN environment variable"
    echo "Usage: DISCORD_TOKEN=your_token $0"
    exit 1
fi

# Run main function
main "$@"
