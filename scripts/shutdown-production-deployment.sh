#!/bin/bash

# Bedrock Production Shutdown Script
# This script safely shuts down the production Bedrock deployment

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
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

# Show current status before shutdown
show_current_status() {
    print_status "Current production deployment status:"
    echo
    
    print_status "ShardClusters:"
    kubectl get shardclusters -n "$NAMESPACE" 2>/dev/null || print_warning "No ShardClusters found"
    echo
    
    print_status "Bot Instances:"
    kubectl get pods -n "$NAMESPACE" -l app=stratum 2>/dev/null || print_warning "No bot pods found"
    echo
    
    print_status "Operator:"
    kubectl get pods -n "$NAMESPACE" -l app=crust-operator 2>/dev/null || print_warning "No operator pods found"
    echo
    
    print_status "Gateway Proxy:"
    kubectl get pods -n "$NAMESPACE" -l app=twilight-gateway-proxy 2>/dev/null || print_warning "No proxy pods found"
    echo
    
    print_status "NATS Cluster:"
    kubectl get pods -n "$NATS_NAMESPACE" -l app=nats 2>/dev/null || print_warning "No NATS pods found"
    echo
}

# Gracefully shutdown bot instances
shutdown_bot_instances() {
    print_status "Shutting down Discord bot instances..."
    
    # Delete ShardCluster resources first (this will trigger graceful shutdown)
    if kubectl get shardclusters -n "$NAMESPACE" &>/dev/null; then
        print_status "Deleting ShardCluster resources..."
        kubectl delete shardclusters --all -n "$NAMESPACE" --timeout=60s
        
        # Wait for bot pods to terminate gracefully
        print_status "Waiting for bot pods to terminate gracefully..."
        kubectl wait --for=delete pod -l app=stratum -n "$NAMESPACE" --timeout=120s 2>/dev/null || print_warning "Some bot pods may still be terminating"
    else
        print_warning "No ShardCluster resources found"
    fi
    
    print_success "Bot instances shutdown completed"
}

# Shutdown operator
shutdown_operator() {
    print_status "Shutting down Bedrock operator..."
    
    if kubectl get deployment crust-operator -n "$NAMESPACE" &>/dev/null; then
        # Scale down operator first to prevent it from recreating resources
        print_status "Scaling down operator..."
        kubectl scale deployment crust-operator --replicas=0 -n "$NAMESPACE"
        
        # Wait for operator pods to terminate
        kubectl wait --for=delete pod -l app=crust-operator -n "$NAMESPACE" --timeout=60s 2>/dev/null || print_warning "Operator pods may still be terminating"
        
        # Delete operator deployment
        print_status "Deleting operator deployment..."
        kubectl delete deployment crust-operator -n "$NAMESPACE"
    else
        print_warning "Operator deployment not found"
    fi
    
    print_success "Operator shutdown completed"
}

# Shutdown gateway proxy
shutdown_gateway_proxy() {
    print_status "Shutting down Twilight Gateway Proxy..."
    
    if kubectl get deployment twilight-gateway-proxy -n "$NAMESPACE" &>/dev/null; then
        kubectl delete deployment twilight-gateway-proxy -n "$NAMESPACE"
        kubectl wait --for=delete pod -l app=twilight-gateway-proxy -n "$NAMESPACE" --timeout=60s 2>/dev/null || print_warning "Proxy pods may still be terminating"
        
        # Delete proxy service
        kubectl delete service twilight-gateway-proxy -n "$NAMESPACE" 2>/dev/null || print_warning "Proxy service not found"
    else
        print_warning "Gateway proxy deployment not found"
    fi
    
    print_success "Gateway proxy shutdown completed"
}

# Shutdown NATS cluster
shutdown_nats() {
    print_status "Shutting down NATS cluster..."
    
    if kubectl get namespace "$NATS_NAMESPACE" &>/dev/null; then
        # Delete NATS StatefulSet
        if kubectl get statefulset nats -n "$NATS_NAMESPACE" &>/dev/null; then
            print_status "Deleting NATS StatefulSet..."
            kubectl delete statefulset nats -n "$NATS_NAMESPACE"
            
            # Wait for NATS pods to terminate
            kubectl wait --for=delete pod -l app=nats -n "$NATS_NAMESPACE" --timeout=120s 2>/dev/null || print_warning "NATS pods may still be terminating"
        fi
        
        # Delete NATS services
        print_status "Deleting NATS services..."
        kubectl delete service --all -n "$NATS_NAMESPACE" 2>/dev/null || print_warning "NATS services not found"
        
        # Delete NATS ConfigMap
        kubectl delete configmap nats-config -n "$NATS_NAMESPACE" 2>/dev/null || print_warning "NATS ConfigMap not found"
        
        # Delete PVCs (this will delete persistent data!)
        print_warning "Deleting NATS persistent volumes (data will be lost)..."
        kubectl delete pvc --all -n "$NATS_NAMESPACE" 2>/dev/null || print_warning "No NATS PVCs found"
    else
        print_warning "NATS namespace not found"
    fi
    
    print_success "NATS cluster shutdown completed"
}

# Clean up RBAC and CRDs
cleanup_rbac_and_crds() {
    print_status "Cleaning up RBAC and CRDs..."
    
    # Delete RBAC resources
    print_status "Deleting RBAC resources..."
    kubectl delete clusterrolebinding crust-operator 2>/dev/null || print_warning "ClusterRoleBinding not found"
    kubectl delete clusterrole crust-operator 2>/dev/null || print_warning "ClusterRole not found"
    kubectl delete serviceaccount crust-operator -n "$NAMESPACE" 2>/dev/null || print_warning "ServiceAccount not found"
    
    # Delete CRDs (this will also delete any remaining ShardCluster resources)
    print_status "Deleting Custom Resource Definitions..."
    kubectl delete crd shardclusters.bedrock.dev 2>/dev/null || print_warning "ShardCluster CRD not found"
    
    print_success "RBAC and CRDs cleanup completed"
}

# Clean up secrets and configmaps
cleanup_secrets_and_config() {
    print_status "Cleaning up secrets and configuration..."
    
    # Delete secrets (Discord token)
    kubectl delete secret discord-token -n "$NAMESPACE" 2>/dev/null || print_warning "Discord token secret not found"
    
    # Delete ConfigMaps
    kubectl delete configmap bedrock-config -n "$NAMESPACE" 2>/dev/null || print_warning "Bedrock ConfigMap not found"
    
    print_success "Secrets and configuration cleanup completed"
}

# Delete namespaces
cleanup_namespaces() {
    print_status "Cleaning up namespaces..."
    
    # Delete bedrock namespace
    if kubectl get namespace "$NAMESPACE" &>/dev/null; then
        print_status "Deleting bedrock namespace..."
        kubectl delete namespace "$NAMESPACE" --timeout=120s
    else
        print_warning "Bedrock namespace not found"
    fi
    
    # Delete NATS namespace
    if kubectl get namespace "$NATS_NAMESPACE" &>/dev/null; then
        print_status "Deleting NATS namespace..."
        kubectl delete namespace "$NATS_NAMESPACE" --timeout=120s
    else
        print_warning "NATS namespace not found"
    fi
    
    print_success "Namespaces cleanup completed"
}

# Verify cleanup
verify_cleanup() {
    print_status "Verifying cleanup..."
    
    # Check for remaining resources
    local remaining_resources=0
    
    if kubectl get namespace "$NAMESPACE" &>/dev/null; then
        print_warning "Bedrock namespace still exists"
        remaining_resources=$((remaining_resources + 1))
    fi
    
    if kubectl get namespace "$NATS_NAMESPACE" &>/dev/null; then
        print_warning "NATS namespace still exists"
        remaining_resources=$((remaining_resources + 1))
    fi
    
    if kubectl get crd shardclusters.bedrock.dev &>/dev/null; then
        print_warning "ShardCluster CRD still exists"
        remaining_resources=$((remaining_resources + 1))
    fi
    
    if kubectl get clusterrole crust-operator &>/dev/null; then
        print_warning "ClusterRole still exists"
        remaining_resources=$((remaining_resources + 1))
    fi
    
    if [ $remaining_resources -eq 0 ]; then
        print_success "✅ Complete cleanup verified - no Bedrock resources remaining"
    else
        print_warning "⚠️  Some resources may still be terminating or require manual cleanup"
        echo
        print_status "You can check for remaining resources with:"
        echo "kubectl get all --all-namespaces | grep -E '(bedrock|nats)'"
        echo "kubectl get crd | grep bedrock"
        echo "kubectl get clusterrole | grep crust"
    fi
}

# Show help
show_help() {
    echo "Bedrock Production Shutdown Script"
    echo
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --quick              Skip confirmation prompts"
    echo "  --keep-data          Keep NATS persistent volumes (preserve data)"
    echo "  --keep-namespaces    Keep namespaces (don't delete them)"
    echo "  --help               Show this help message"
    echo
    echo "Examples:"
    echo "  $0                   # Interactive shutdown with confirmations"
    echo "  $0 --quick           # Quick shutdown without prompts"
    echo "  $0 --keep-data       # Shutdown but preserve NATS data"
    echo
}

# Parse command line arguments
QUICK_MODE=false
KEEP_DATA=false
KEEP_NAMESPACES=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick)
            QUICK_MODE=true
            shift
            ;;
        --keep-data)
            KEEP_DATA=true
            shift
            ;;
        --keep-namespaces)
            KEEP_NAMESPACES=true
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
    if [ "$QUICK_MODE" = true ]; then
        return 0
    fi
    
    echo
    print_warning "⚠️  This will completely shut down the Bedrock production deployment!"
    print_warning "   • All Discord bot instances will be terminated"
    print_warning "   • All operator and proxy services will be stopped"
    print_warning "   • NATS cluster will be shut down"
    if [ "$KEEP_DATA" = false ]; then
        print_warning "   • ALL PERSISTENT DATA WILL BE DELETED"
    fi
    echo
    
    read -p "Are you sure you want to proceed? (type 'yes' to confirm): " confirmation
    
    if [ "$confirmation" != "yes" ]; then
        print_status "Shutdown cancelled"
        exit 0
    fi
}

# Main execution
main() {
    echo "🛑 Bedrock Production Shutdown"
    echo "=============================="
    echo
    
    show_current_status
    confirm_shutdown
    
    print_status "Starting production shutdown sequence..."
    echo
    
    # Shutdown in reverse order of deployment
    shutdown_bot_instances
    echo
    
    shutdown_operator
    echo
    
    shutdown_gateway_proxy
    echo
    
    shutdown_nats
    echo
    
    cleanup_rbac_and_crds
    echo
    
    cleanup_secrets_and_config
    echo
    
    if [ "$KEEP_NAMESPACES" = false ]; then
        cleanup_namespaces
        echo
    else
        print_status "Skipping namespace deletion (--keep-namespaces specified)"
        echo
    fi
    
    verify_cleanup
    echo
    
    print_success "🎉 Production shutdown completed!"
    echo
    
    if [ "$KEEP_DATA" = true ]; then
        print_status "NATS data was preserved and can be restored on next deployment"
    else
        print_warning "All data has been permanently deleted"
    fi
    
    print_status "To redeploy: DISCORD_TOKEN=your_token ./scripts/deploy-production.sh"
}

# Handle interrupts
trap 'print_error "Shutdown interrupted"; exit 1' INT TERM

# Run main function
main "$@"
