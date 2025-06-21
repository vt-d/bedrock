#!/bin/bash

# Cleanup script for NATS development deployment
set -e

NAMESPACE="nats-dev"
RELEASE_NAME="nats-dev"

echo "🧹 Cleaning up NATS development deployment..."

# Uninstall Helm release
echo "📦 Uninstalling Helm release: $RELEASE_NAME"
helm uninstall $RELEASE_NAME --namespace $NAMESPACE || echo "Release not found or already uninstalled"

# Delete persistent volume claims (optional - comment out to keep data)
echo "💾 Cleaning up persistent volume claims..."
kubectl delete pvc -n $NAMESPACE -l app.kubernetes.io/instance=$RELEASE_NAME || echo "No PVCs found"

# Delete namespace (optional - comment out to keep namespace)
echo "🗑️  Deleting namespace: $NAMESPACE"
kubectl delete namespace $NAMESPACE || echo "Namespace not found or already deleted"

echo "✅ Cleanup completed!"
