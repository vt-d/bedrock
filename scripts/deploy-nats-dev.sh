#!/bin/bash

# Development deployment script for NATS cluster
set -e

NAMESPACE="nats-dev"
RELEASE_NAME="nats-dev"
CHART_PATH="./helm/nats-cluster"

echo "🚀 Deploying NATS cluster for development..."

# Create namespace if it doesn't exist
echo "📦 Creating namespace: $NAMESPACE"
kubectl create namespace $NAMESPACE --dry-run=client -o yaml | kubectl apply -f -

# Install or upgrade the Helm chart
echo "⚡ Installing NATS cluster..."
helm upgrade --install $RELEASE_NAME $CHART_PATH \
  --namespace $NAMESPACE \
  --values $CHART_PATH/values-dev.yaml \
  --wait \
  --timeout 300s

echo "✅ NATS cluster deployed successfully!"

# Get service information
echo ""
echo "📋 Service Information:"
kubectl get svc -n $NAMESPACE

# Get pod information
echo ""
echo "🔍 Pod Information:"
kubectl get pods -n $NAMESPACE

# Show connection information
echo ""
echo "🔗 Connection Information:"
echo "  Internal cluster access: nats-dev.nats-dev.svc.cluster.local:4222"
echo "  Port forward command: kubectl port-forward -n $NAMESPACE svc/nats-dev 4222:4222"
echo "  Monitor URL (after port-forward): http://localhost:8222"

# Get NodePort if available
NODEPORT=$(kubectl get svc -n $NAMESPACE $RELEASE_NAME -o jsonpath='{.spec.ports[?(@.name=="client")].nodePort}' 2>/dev/null || echo "")
if [ ! -z "$NODEPORT" ]; then
    echo "  External access (NodePort): <node-ip>:$NODEPORT"
fi

echo ""
echo "🎯 Quick test commands:"
echo "  kubectl port-forward -n $NAMESPACE svc/nats-dev 4222:4222 &"
echo "  nats pub test.subject 'Hello World' --server nats://localhost:4222"
echo "  nats sub test.subject --server nats://localhost:4222"
