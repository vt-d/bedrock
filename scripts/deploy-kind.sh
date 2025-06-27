#!/bin/bash

# Bedrock Discord Bot Operator - Kind Deployment Script
# This script automates the complete deployment of the Bedrock operator and NATS cluster on Kind
# AI Generated Script, before people roast me for not writing it myself

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CLUSTER_NAME="bedrock-discord-bot"
NAMESPACE="bedrock"
NATS_NAMESPACE="nats-system"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DISCORD_TOKEN=""
NO_PORT_MAPPING=false

# Print colored output
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

# Check if required tools are installed
check_prerequisites() {
    print_status "Checking prerequisites..."
    
    local missing_tools=()
    
    if ! command -v kind &> /dev/null; then
        missing_tools+=("kind")
    fi
    
    if ! command -v kubectl &> /dev/null; then
        missing_tools+=("kubectl")
    fi
    
    if ! command -v docker &> /dev/null; then
        missing_tools+=("docker")
    fi
    
    if [ ${#missing_tools[@]} -ne 0 ]; then
        print_error "Missing required tools: ${missing_tools[*]}"
        print_error "Please install them and try again"
        exit 1
    fi
    
    # Check Docker permissions
    if ! docker ps &>/dev/null; then
        print_warning "Docker permission issue detected"
        print_warning "If you get permission errors, try one of these solutions:"
        print_warning "1. Add your user to the docker group: sudo usermod -aG docker \$USER"
        print_warning "2. Run this script with sudo: sudo $0"
        print_warning "3. Use rootless Docker"
        echo
        print_status "Attempting to continue..."
    fi
    
    print_success "All prerequisites are installed"
}

# Get Discord token from user
get_discord_token() {
    if [ -z "$DISCORD_TOKEN" ]; then
        echo
        print_warning "Discord bot token is required for deployment"
        echo "You can get a token from: https://discord.com/developers/applications"
        echo
        read -s -p "Enter your Discord bot token: " DISCORD_TOKEN
        echo
        
        if [ -z "$DISCORD_TOKEN" ]; then
            print_error "Discord token is required"
            exit 1
        fi
    fi
}

# Create Kind cluster
create_kind_cluster() {
    print_status "Creating Kind cluster: $CLUSTER_NAME"
    
    if kind get clusters | grep -q "^$CLUSTER_NAME$"; then
        print_warning "Cluster $CLUSTER_NAME already exists"
        read -p "Do you want to delete and recreate it? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            print_status "Deleting existing cluster..."
            kind delete cluster --name "$CLUSTER_NAME"
        else
            print_status "Using existing cluster"
            return
        fi
    fi
    
    if [ "$NO_PORT_MAPPING" = true ]; then
        print_status "Creating cluster without port mappings..."
        cat <<EOF | kind create cluster --name "$CLUSTER_NAME" --config=-
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
EOF
    else
        # First try with port mappings for convenience
        print_status "Creating cluster with NATS port mappings..."
        if ! cat <<EOF | kind create cluster --name "$CLUSTER_NAME" --config=- 2>/dev/null
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  extraPortMappings:
  - containerPort: 4222
    hostPort: 4222
    protocol: TCP
    # NATS client port for optional external access
  - containerPort: 8222
    hostPort: 8222
    protocol: TCP
    # NATS monitoring port
EOF
        then
            print_warning "Port mapping failed (ports likely in use). Creating cluster without port mappings..."
            # Fallback: create cluster without port mappings
            cat <<EOF | kind create cluster --name "$CLUSTER_NAME" --config=-
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
EOF
            print_warning "NATS will only be accessible via kubectl port-forward"
        fi
    fi
    
    print_success "Kind cluster created successfully"
    
    # Ensure kubectl context is set to the new cluster
    print_status "Setting kubectl context to Kind cluster..."
    kubectl cluster-info --context "kind-$CLUSTER_NAME"
    kubectl config use-context "kind-$CLUSTER_NAME"
}

# Verify kubectl connection
verify_kubectl_connection() {
    print_status "Verifying kubectl connection to cluster..."
    
    local retries=0
    local max_retries=10
    
    while [ $retries -lt $max_retries ]; do
        if kubectl get nodes &>/dev/null; then
            print_success "kubectl connected successfully"
            
            # Show cluster info
            print_status "Cluster nodes:"
            kubectl get nodes
            echo
            
            return 0
        fi
        
        print_warning "kubectl connection failed, retrying in 2 seconds... ($((retries + 1))/$max_retries)"
        sleep 2
        retries=$((retries + 1))
    done
    
    print_error "Failed to connect to cluster after $max_retries attempts"
    print_error "Try running: kubectl config use-context kind-$CLUSTER_NAME"
    exit 1
}

# Pre-pull NATS image to speed up deployment
pre_pull_images() {
    print_status "Pre-pulling NATS image to speed up deployment..."
    
    # Pull NATS image on the Kind node
    docker pull nats:2.10-alpine
    kind load docker-image nats:2.10-alpine --name "$CLUSTER_NAME"
    
    print_success "Images pre-pulled successfully"
}

# Build and load Docker images
build_and_load_images() {
    print_status "Building Docker images..."
    
    cd "$PROJECT_ROOT"
    
    # Build crust operator
    print_status "Building Bedrock operator (crust) image..."
    docker build -f Dockerfile.crust -t bedrock-operator:latest .
    kind load docker-image bedrock-operator:latest --name "$CLUSTER_NAME"
    
    # Build stratum bot
    print_status "Building Bedrock bot (stratum) image..."
    docker build -f Dockerfile.stratum -t bedrock-bot:latest .
    kind load docker-image bedrock-bot:latest --name "$CLUSTER_NAME"
    
    # Build your custom image (uncomment and modify as needed)
    # print_status "Building your custom image..."
    # docker build -f Dockerfile.your-app -t your-app:latest .
    # kind load docker-image your-app:latest --name "$CLUSTER_NAME"
    
    print_success "Docker images built and loaded"
}

# Deploy NATS cluster
deploy_nats() {
    print_status "Deploying NATS cluster..."
    
    kubectl apply -f "$PROJECT_ROOT/k8s/nats-cluster.yaml"
    
    print_status "Waiting for NATS StatefulSet to be created..."
    # Wait for StatefulSet to exist and be observed by the controller
    local retries=0
    while [ $retries -lt 30 ]; do
        if kubectl get statefulset/nats -n "$NATS_NAMESPACE" &>/dev/null; then
            print_success "StatefulSet created"
            break
        fi
        sleep 2
        retries=$((retries + 1))
    done
    
    if [ $retries -eq 30 ]; then
        print_error "StatefulSet creation failed"
        kubectl get all -n "$NATS_NAMESPACE"
        exit 1
    fi
    
    print_status "Waiting for NATS pods to be scheduled and ready (this may take a few minutes)..."
    # Check pod creation progress
    local pod_retries=0
    while [ $pod_retries -lt 60 ]; do
        local ready_pods=$(kubectl get pods -n "$NATS_NAMESPACE" -l app=nats --no-headers 2>/dev/null | grep "Running\|Ready" | wc -l)
        local total_pods=$(kubectl get pods -n "$NATS_NAMESPACE" -l app=nats --no-headers 2>/dev/null | wc -l)
        
        if [ "$ready_pods" -eq 3 ] && [ "$total_pods" -eq 3 ]; then
            print_success "All NATS pods are running"
            break
        elif [ "$total_pods" -gt 0 ]; then
            print_status "NATS pods status: $ready_pods/$total_pods ready"
            kubectl get pods -n "$NATS_NAMESPACE" -l app=nats --no-headers 2>/dev/null || true
        else
            print_status "Waiting for NATS pods to be scheduled..."
        fi
        
        sleep 5
        pod_retries=$((pod_retries + 1))
    done
    
    # Final check with kubectl wait for any remaining pods
    if ! kubectl wait --for=condition=ready pod -l app=nats -n "$NATS_NAMESPACE" --timeout=60s 2>/dev/null; then
        print_warning "Some pods may still be starting. Current status:"
        kubectl get pods -n "$NATS_NAMESPACE" -l app=nats
        echo
        print_status "Pod details:"
        kubectl describe pods -n "$NATS_NAMESPACE" -l app=nats
        echo
        print_status "Recent events:"
        kubectl get events -n "$NATS_NAMESPACE" --sort-by='.lastTimestamp' | tail -10
        
        # Don't exit, let the user decide if they want to continue
        print_warning "NATS pods are not fully ready, but continuing deployment..."
        print_warning "You may need to troubleshoot NATS issues manually"
    else
        print_success "NATS cluster deployed successfully"
    fi
}

# Create namespace and secrets
setup_namespace_and_secrets() {
    print_status "Setting up namespace and secrets..."
    
    # Create namespace
    kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f -
    
    # Create Discord token secret
    kubectl create secret generic discord-token \
        --from-literal=token="$DISCORD_TOKEN" \
        --namespace="$NAMESPACE" \
        --dry-run=client -o yaml | kubectl apply -f -
    
    print_success "Namespace and secrets created"
}

# Deploy CRD
deploy_crd() {
    print_status "Deploying ShardCluster CRD..."
    
    kubectl apply -f "$PROJECT_ROOT/crd/shardcluster-crd.yaml"
    
    print_success "CRD deployed successfully"
}

# Deploy RBAC
deploy_rbac() {
    print_status "Deploying RBAC..."
    
    kubectl apply -f "$PROJECT_ROOT/k8s/rbac.yaml"
    
    print_success "RBAC deployed successfully"
}

# Deploy operator
deploy_operator() {
    print_status "Deploying Bedrock operator (crust)..."
    
    # Update the deployment to use our built image and correct NATS URL
    cat "$PROJECT_ROOT/k8s/operator-deployment.yaml" | \
    sed 's|image: .*|image: bedrock-operator:latest|g' | \
    sed 's|imagePullPolicy: .*|imagePullPolicy: Never|g' | \
    sed 's|nats://[^"]*|nats://nats-cluster.nats-system.svc.cluster.local:4222|g' | \
    kubectl apply -f -
    
    print_status "Waiting for operator to be ready..."
    kubectl wait --for=condition=available deployment/crust-operator -n bedrock --timeout=300s
    
    print_success "Bedrock operator deployed successfully"
}

# Deploy example ShardCluster
deploy_example_cluster() {
    print_status "Deploying example Bedrock bot cluster..."
    
    # Create example ShardCluster with correct image and settings
    cat <<EOF | kubectl apply -f -
apiVersion: bedrock.dev/v1
kind: ShardCluster
metadata:
  name: bedrock-discord-bot
  namespace: $NAMESPACE
spec:
  discord_token_secret: "discord-token"
  nats_url: "nats://nats-cluster.nats-system.svc.cluster.local:4222"
  image: "bedrock-bot:latest"
  replicas_per_shard_group: 1
  shards_per_replica: 4
  reshard_interval_hours: 24
EOF
    
    print_success "Example Bedrock bot cluster deployed"
}

# Deploy your custom application
run_proxy() {
    print_status "Deploying your application..."
    
    # Apply the deployment which will use the discord-token secret
    kubectl apply -f "$PROJECT_ROOT/k8s/twilight-gateway-proxy.yaml"
    
    print_status "Waiting for your app to be ready..."
    kubectl wait --for=condition=available deployment/twilight-gateway-proxy --timeout=300s -n "$NAMESPACE"
    
    print_success "Your application deployed successfully"
    print_status "Your app has access to DISCORD_TOKEN from the secret"
}

# Show status and useful commands
show_status() {
    print_status "Deployment complete! Here's the status:"
    echo
    
    print_status "NATS Cluster Status:"
    kubectl get pods -n "$NATS_NAMESPACE" -l app=nats
    echo
    
    print_status "Operator Status:"
    kubectl get pods -l app=crust-operator -n bedrock
    echo
    
    print_status "Bedrock Bot Status:"
    kubectl get pods -n "$NAMESPACE" -l app=stratum
    echo
    
    print_status "ShardCluster Status:"
    kubectl get shardclusters -n "$NAMESPACE"
    echo
    
    print_success "Useful commands:"
    echo "# View operator logs:"
    echo "kubectl logs -l app=crust-operator -n bedrock -f"
    echo
    echo "# View bot instance logs:"
    echo "kubectl logs -n $NAMESPACE -l app=stratum -f"
    echo
    echo "# View NATS logs:"
    echo "kubectl logs -n $NATS_NAMESPACE -l app=nats -f"
    echo
    echo "# Port forward NATS monitoring:"
    echo "kubectl port-forward -n $NATS_NAMESPACE svc/nats-cluster 8222:8222"
    echo "# Then visit: http://localhost:8222"
    echo
    echo "# Fix kubectl context if needed:"
    echo "kubectl config use-context kind-$CLUSTER_NAME"
    echo
    echo "# Delete everything:"
    echo "kind delete cluster --name $CLUSTER_NAME"
    echo "# OR use the shutdown script:"
    echo "./scripts/shutdown-deployment.sh"
    echo
}

# Cleanup function
cleanup() {
    print_status "Cleaning up..."
    kind delete cluster --name "$CLUSTER_NAME" 2>/dev/null || true
    print_success "Cleanup complete"
}

# Show help
show_help() {
    echo "Bedrock Discord Bot Operator - Kind Deployment Script"
    echo
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --token TOKEN        Discord bot token (can also be set via DISCORD_TOKEN env var)"
    echo "  --cluster NAME       Kind cluster name (default: bedrock-discord-bot)"
    echo "  --namespace NAME     Kubernetes namespace (default: bedrock)"
    echo "  --no-port-mapping    Create cluster without host port mappings (avoids port conflicts)"
    echo "  --cleanup            Delete the Kind cluster and exit"
    echo "  --help              Show this help message"
    echo
    echo "Examples:"
    echo "  $0 --token your_discord_token_here"
    echo "  DISCORD_TOKEN=your_token $0"
    echo "  $0 --no-port-mapping  # Avoids port 80/443/4222/8222 conflicts"
    echo "  $0 --cleanup"
    echo
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --token)
            DISCORD_TOKEN="$2"
            shift 2
            ;;
        --cluster)
            CLUSTER_NAME="$2"
            shift 2
            ;;
        --namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        --no-port-mapping)
            NO_PORT_MAPPING=true
            shift
            ;;
        --cleanup)
            cleanup
            exit 0
            ;;
        --help)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Main execution
main() {
    echo "🚀 Bedrock Discord Bot Operator - Kind Deployment"
    echo "=============================================="
    echo
    
    check_prerequisites
    get_discord_token
    
    print_status "Starting deployment with cluster: $CLUSTER_NAME"
    print_status "Namespace: $NAMESPACE"
    echo
    
    create_kind_cluster
    verify_kubectl_connection
    pre_pull_images
    build_and_load_images
    deploy_nats
    setup_namespace_and_secrets
    deploy_crd
    deploy_rbac
    deploy_operator
    deploy_example_cluster
    run_proxy
    
    echo
    print_success "🎉 Deployment completed successfully!"
    echo
    
    # Wait a moment for things to settle
    sleep 5
    show_status
}

# Handle interrupts
trap 'print_error "Deployment interrupted"; exit 1' INT TERM

# Run main function
main "$@"
