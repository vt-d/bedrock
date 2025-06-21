#!/bin/bash

# Setup script for local Kubernetes development environment
set -e

CLUSTER_NAME="bedrock-dev"

echo "🚀 Setting up Kubernetes development environment..."

# Check if Docker is running
if ! docker info &>/dev/null; then
    echo "❌ Docker is not running. Please start Docker first."
    exit 1
fi

# Create kind cluster if it doesn't exist
if ! kind get clusters | grep -q "^${CLUSTER_NAME}$"; then
    echo "📦 Creating kind cluster: $CLUSTER_NAME"
    
    # Create cluster config for better development experience
    cat > /tmp/kind-config.yaml << EOF
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  kubeadmConfigPatches:
  - |
    kind: InitConfiguration
    nodeRegistration:
      kubeletExtraArgs:
        node-labels: "ingress-ready=true"
  extraPortMappings:
  - containerPort: 80
    hostPort: 80
    protocol: TCP
  - containerPort: 443
    hostPort: 443
    protocol: TCP
  - containerPort: 30000
    hostPort: 30000
    protocol: TCP
  - containerPort: 30001
    hostPort: 30001
    protocol: TCP
EOF
    
    kind create cluster --name $CLUSTER_NAME --config /tmp/kind-config.yaml
    rm /tmp/kind-config.yaml
else
    echo "✅ Kind cluster '$CLUSTER_NAME' already exists"
fi

# Set kubectl context
echo "🔧 Setting kubectl context..."
kubectl config use-context kind-$CLUSTER_NAME

# Verify cluster is working
echo "✅ Verifying cluster..."
kubectl cluster-info
kubectl get nodes

# Install a default storage class (if not present)
if ! kubectl get storageclass | grep -q "standard"; then
    echo "📦 Installing local storage class..."
    kubectl apply -f - << EOF
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: standard
  annotations:
    storageclass.kubernetes.io/is-default-class: "true"
provisioner: rancher.io/local-path
volumeBindingMode: WaitForFirstConsumer
reclaimPolicy: Delete
EOF
fi

echo ""
echo "🎯 Kubernetes development environment ready!"
echo "  Cluster name: $CLUSTER_NAME"
echo "  Context: kind-$CLUSTER_NAME"
echo "  Nodes: $(kubectl get nodes --no-headers | wc -l)"
echo ""
echo "Next steps:"
echo "  1. Deploy NATS: make dev-deploy"
echo "  2. Set up port forwarding: make dev-port-forward"
echo ""
echo "Useful commands:"
echo "  kubectl get all --all-namespaces  # See all resources"
echo "  kind delete cluster --name $CLUSTER_NAME  # Delete cluster when done"
