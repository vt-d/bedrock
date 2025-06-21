#!/bin/bash

# Port forwarding script for easy NATS access during development
set -e

NAMESPACE="nats-dev"
RELEASE_NAME="nats-dev"
SERVICE_NAME="nats-dev-nats-cluster"
CLIENT_PORT="4222"
MONITOR_PORT="8222"

echo "🔗 Setting up port forwarding for NATS development..."

# Check if the service exists
if ! kubectl get svc -n $NAMESPACE $SERVICE_NAME &>/dev/null; then
    echo "❌ NATS service not found. Please deploy first using: ./scripts/deploy-nats-dev.sh"
    exit 1
fi

echo "📡 Port forwarding NATS client port: localhost:$CLIENT_PORT"
echo "📊 Port forwarding NATS monitor port: localhost:$MONITOR_PORT"
echo ""
echo "💡 You can now connect to NATS at: nats://localhost:$CLIENT_PORT"
echo "🔍 Monitor dashboard available at: http://localhost:$MONITOR_PORT"
echo ""
echo "Press Ctrl+C to stop port forwarding..."

# Start port forwarding
kubectl port-forward -n $NAMESPACE svc/$SERVICE_NAME $CLIENT_PORT:$CLIENT_PORT $MONITOR_PORT:$MONITOR_PORT
