#!/bin/bash

# Quick test script for NATS + Rust integration
set -e

echo "🧪 Testing NATS + Rust Discord Bot Integration"
echo ""

# Check if NATS is running
echo "1. Checking NATS deployment..."
if kubectl get pods -n nats-dev | grep -q "Running"; then
    echo "✅ NATS is running"
else
    echo "❌ NATS is not running. Run 'make dev-deploy' first"
    exit 1
fi

# Start port forwarding in background
echo ""
echo "2. Setting up port forwarding..."
kubectl port-forward -n nats-dev svc/nats-dev-nats-cluster 4222:4222 &
PORT_FORWARD_PID=$!

# Wait for port forwarding to establish
sleep 3

# Test NATS connection with nats CLI (if available)
echo ""
echo "3. Testing NATS connection..."
if command -v nats &> /dev/null; then
    echo "📤 Publishing test message..."
    nats pub discord.test "Hello from test script!" --server nats://localhost:4222
    echo "✅ NATS connection test successful"
else
    echo "ℹ️  nats CLI not available, skipping connection test"
    echo "   Install with: curl -sf https://binaries.nats.dev/nats-io/nats/v0.0.35/nats-0.0.35-linux-amd64.tar.gz | tar -zxf - && sudo mv nats /usr/local/bin/"
fi

# Build and test Rust application
echo ""
echo "4. Building Rust application..."
cd /home/vt/dev/bedrock/bot
if cargo build --bin stratum; then
    echo "✅ Rust application built successfully"
    echo ""
    echo "🚀 You can now run your bot with:"
    echo "   DISCORD_TOKEN=your_token cargo run --bin stratum"
    echo ""
    echo "📡 NATS will be available at: nats://localhost:4222"
    echo "🔍 Monitor dashboard at: http://localhost:8222"
else
    echo "❌ Failed to build Rust application"
fi

# Cleanup
echo ""
echo "🧹 Cleaning up port forwarding..."
kill $PORT_FORWARD_PID 2>/dev/null || true

echo ""
echo "✅ Test completed!"
echo ""
echo "Next steps:"
echo "  1. Set your Discord token: export DISCORD_TOKEN=your_actual_token"
echo "  2. Start port forwarding: make dev-port-forward"
echo "  3. Run your bot: cd bot && cargo run --bin stratum"
