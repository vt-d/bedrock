#!/bin/bash

# Bedrock Discord Bot Operator - Shutdown Script
# This script cleanly removes the entire Bedrock deployment from Kind

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

# Show help
show_help() {
    echo "Bedrock Discord Bot Operator - Shutdown Script"
    echo
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --cluster NAME       Kind cluster name (default: bedrock-discord-bot)"
    echo "  --namespace NAME     Kubernetes namespace (default: bedrock)"
    echo "  --keep-cluster       Only remove Bedrock components, keep the cluster"
    echo "  --force              Skip confirmation prompts"
    echo "  --help              Show this help message"
    echo
    echo "Examples:"
    echo "  $0                   # Full shutdown with confirmation"
    echo "  $0 --force           # Full shutdown without confirmation"
    echo "  $0 --keep-cluster    # Remove only Bedrock components"
    echo
}

# Parse command line arguments
KEEP_CLUSTER=false
FORCE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --cluster)
            CLUSTER_NAME="$2"
            shift 2
            ;;
        --namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        --keep-cluster)
            KEEP_CLUSTER=true
            shift
            ;;
        --force)
            FORCE=true
            shift
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

# Confirmation prompt
confirm_shutdown() {
    if [ "$FORCE" = true ]; then
        return 0
    fi
    
    echo "🛑 Bedrock Discord Bot Operator - Shutdown Confirmation"
    echo "======================================================"
    echo
    echo "This will remove:"
    if [ "$KEEP_CLUSTER" = false ]; then
        echo "  ❌ Kind cluster: $CLUSTER_NAME (COMPLETE DELETION)"
        echo "  ❌ All Docker containers and volumes"
        echo "  ❌ All Kubernetes resources"
    else
        echo "  ❌ Bedrock bot instances and operator"
        echo "  ❌ NATS cluster"
        echo "  ❌ Custom Resource Definitions"
        echo "  ❌ RBAC resources"
        echo "  ❌ Namespaces: $NAMESPACE, $NATS_NAMESPACE"
        echo "  ✅ Kind cluster will be preserved"
    fi
    echo
    read -p "Are you sure you want to proceed? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_status "Shutdown cancelled"
        exit 0
    fi
}

# Check if cluster exists
check_cluster() {
    if ! command -v kind &> /dev/null; then
        print_error "Kind is not installed"
        exit 1
    fi
    
    if ! kind get clusters 2>/dev/null | grep -q "^$CLUSTER_NAME$"; then
        print_warning "Cluster $CLUSTER_NAME does not exist"
        if [ "$KEEP_CLUSTER" = false ]; then
            print_success "Nothing to clean up"
            exit 0
        else
            print_error "Cannot clean components from non-existent cluster"
            exit 1
        fi
    fi
}

# Remove Bedrock bot instances
remove_bot_instances() {
    print_status "Removing Bedrock bot instances..."
    
    if kubectl get namespace "$NAMESPACE" &>/dev/null; then
        # Delete ShardClusters first (this should clean up managed resources)
        if kubectl get shardclusters -n "$NAMESPACE" &>/dev/null; then
            print_status "Deleting ShardCluster resources..."
            kubectl delete shardclusters --all -n "$NAMESPACE" --timeout=60s || true
        fi
        
        # Delete any remaining deployments/pods
        print_status "Cleaning up remaining bot resources..."
        kubectl delete deployments,pods,services,configmaps,secrets -l app=stratum -n "$NAMESPACE" --timeout=60s || true
        
        print_success "Bot instances removed"
    else
        print_warning "Namespace $NAMESPACE not found"
    fi
}

# Remove operator
remove_operator() {
    print_status "Removing Bedrock operator..."
    
    # Delete operator deployment
    kubectl delete deployment crust-operator --timeout=60s || true
    
    # Delete operator pods if any are stuck
    kubectl delete pods -l app=crust-operator --timeout=30s || true
    
    print_success "Operator removed"
}

# Remove NATS cluster
remove_nats() {
    print_status "Removing NATS cluster..."
    
    if kubectl get namespace "$NATS_NAMESPACE" &>/dev/null; then
        # Delete NATS resources
        kubectl delete statefulset,pods,services,configmaps,persistentvolumeclaims -l app=nats -n "$NATS_NAMESPACE" --timeout=60s || true
        
        # Delete the namespace
        print_status "Deleting NATS namespace..."
        kubectl delete namespace "$NATS_NAMESPACE" --timeout=60s || true
        
        print_success "NATS cluster removed"
    else
        print_warning "NATS namespace not found"
    fi
}

# Remove CRDs and RBAC
remove_crds_and_rbac() {
    print_status "Removing CRDs and RBAC resources..."
    
    # Delete CRDs
    kubectl delete crd shardclusters.bedrock.dev --timeout=30s || true
    
    # Delete RBAC
    kubectl delete clusterrolebinding crust-operator --timeout=30s || true
    kubectl delete clusterrole crust-operator --timeout=30s || true
    kubectl delete serviceaccount crust-operator --timeout=30s || true
    
    print_success "CRDs and RBAC removed"
}

# Remove namespace
remove_namespace() {
    print_status "Removing Bedrock namespace..."
    
    if kubectl get namespace "$NAMESPACE" &>/dev/null; then
        kubectl delete namespace "$NAMESPACE" --timeout=60s || true
        print_success "Namespace removed"
    else
        print_warning "Namespace $NAMESPACE not found"
    fi
}

# Remove entire cluster
remove_cluster() {
    print_status "Deleting Kind cluster: $CLUSTER_NAME"
    
    kind delete cluster --name "$CLUSTER_NAME"
    
    print_success "Cluster deleted successfully"
}

# Show final status
show_final_status() {
    echo
    print_success "🎉 Shutdown completed successfully!"
    echo
    
    if [ "$KEEP_CLUSTER" = false ]; then
        print_status "The entire Bedrock deployment has been removed"
        print_status "Kind cluster '$CLUSTER_NAME' has been deleted"
        echo
        print_status "To redeploy, run:"
        echo "  ./scripts/deploy-kind.sh --token YOUR_DISCORD_BOT_TOKEN"
    else
        print_status "Bedrock components have been removed from cluster '$CLUSTER_NAME'"
        print_status "The Kind cluster is still running"
        echo
        print_status "Cluster status:"
        kubectl get nodes 2>/dev/null || print_warning "Cannot connect to cluster"
        echo
        print_status "To redeploy Bedrock components, run:"
        echo "  ./scripts/deploy-kind.sh --token YOUR_DISCORD_BOT_TOKEN"
        echo
        print_status "To delete the cluster completely, run:"
        echo "  kind delete cluster --name $CLUSTER_NAME"
    fi
    echo
}

# Main execution
main() {
    echo "🛑 Bedrock Discord Bot Operator - Shutdown"
    echo "=========================================="
    echo
    
    check_cluster
    confirm_shutdown
    
    print_status "Starting shutdown process..."
    print_status "Cluster: $CLUSTER_NAME"
    print_status "Namespace: $NAMESPACE"
    if [ "$KEEP_CLUSTER" = true ]; then
        print_status "Mode: Component cleanup (cluster preserved)"
    else
        print_status "Mode: Full removal (cluster deleted)"
    fi
    echo
    
    # Set kubectl context
    if kubectl config use-context "kind-$CLUSTER_NAME" &>/dev/null; then
        print_success "Connected to cluster"
    else
        print_warning "Could not set kubectl context, proceeding anyway..."
    fi
    
    if [ "$KEEP_CLUSTER" = true ]; then
        # Remove components but keep cluster
        remove_bot_instances
        remove_operator
        remove_nats
        remove_crds_and_rbac
        remove_namespace
    else
        # Remove entire cluster
        remove_cluster
    fi
    
    show_final_status
}

# Handle interrupts
trap 'print_error "Shutdown interrupted"; exit 1' INT TERM

# Run main function
main "$@"
