#!/bin/bash

# Quick diagnostic script for Bedrock deployment issues

echo "🔍 Bedrock Deployment Diagnostics"
echo "================================="

# Check if cluster exists
echo "1. Checking Kind cluster..."
if kind get clusters | grep -q "bedrock-discord-bot"; then
    echo "✅ bedrock-discord-bot cluster exists"
else
    echo "❌ bedrock-discord-bot cluster not found"
    echo "Run: ./scripts/deploy-kind.sh to create it"
    exit 1
fi

# Check kubectl context
echo ""
echo "2. Checking kubectl context..."
current_context=$(kubectl config current-context 2>/dev/null || echo "none")
if [[ "$current_context" == "kind-bedrock-discord-bot" ]]; then
    echo "✅ kubectl context is correct: $current_context"
else
    echo "❌ kubectl context is: $current_context"
    echo "Fix: kubectl config use-context kind-bedrock-discord-bot"
fi

# Check cluster connectivity
echo ""
echo "3. Checking cluster connectivity..."
if kubectl get nodes &>/dev/null; then
    echo "✅ Cluster is accessible"
    kubectl get nodes
else
    echo "❌ Cannot connect to cluster"
    exit 1
fi

# Check namespaces
echo ""
echo "4. Checking namespaces..."
kubectl get namespaces | grep -E "(nats-system|bedrock)" || echo "No bedrock/nats-system namespaces found"

# Check NATS status
echo ""
echo "5. Checking NATS deployment..."
if kubectl get namespace nats-system &>/dev/null; then
    echo "NATS namespace exists"
    echo ""
    echo "NATS StatefulSet:"
    kubectl get statefulset -n nats-system
    echo ""
    echo "NATS Pods:"
    kubectl get pods -n nats-system
    echo ""
    echo "NATS Events (last 10):"
    kubectl get events -n nats-system --sort-by='.lastTimestamp' | tail -10
else
    echo "NATS namespace not found"
fi

# Check operator status
echo ""
echo "6. Checking operator deployment..."
if kubectl get namespace bedrock &>/dev/null; then
    echo "Bedrock namespace exists"
    echo ""
    echo "Operator Deployment:"
    kubectl get deployment -l app=crust-operator 2>/dev/null || echo "No operator deployment found"
    echo ""
    echo "Operator Pods:"
    kubectl get pods -l app=crust-operator 2>/dev/null || echo "No operator pods found"
else
    echo "Bedrock namespace not found"
fi

# Resource usage
echo ""
echo "7. Cluster resource usage..."
kubectl top nodes 2>/dev/null || echo "Metrics not available"

echo ""
echo "🔧 Common fixes:"
echo "- Stuck NATS: kubectl delete pod -n nats-system -l app=nats"
echo "- Reset kubectl: kubectl config use-context kind-bedrock-discord-bot"
echo "- Full restart: ./scripts/deploy-kind.sh --cleanup && ./scripts/deploy-kind.sh --token YOUR_TOKEN"
