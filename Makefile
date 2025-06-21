# NATS Development Makefile

.PHONY: k8s-setup k8s-clean dev-deploy dev-clean dev-port-forward dev-status dev-logs dev-test nats-purge-streams

# Setup local Kubernetes cluster
k8s-setup:
	@echo "🚀 Setting up Kubernetes development environment..."
	@./scripts/setup-k8s-dev.sh

# Clean up Kubernetes cluster
k8s-clean:
	@echo "🧹 Cleaning up Kubernetes cluster..."
	@kind delete cluster --name bedrock-dev || echo "Cluster not found"

# Deploy NATS for development
dev-deploy:
	@echo "🚀 Deploying NATS for development..."
	@./scripts/deploy-nats-dev.sh

# Clean up development deployment  
dev-clean:
	@echo "🧹 Cleaning up NATS development deployment..."
	@./scripts/cleanup-nats-dev.sh

# Set up port forwarding
dev-port-forward:
	@echo "🔗 Setting up port forwarding..."
	@./scripts/port-forward-nats.sh

# Check deployment status
dev-status:
	@echo "📋 NATS Development Status:"
	@kubectl get pods,svc -n nats-dev
	@echo ""
	@echo "🔍 Pod logs (last 10 lines):"
	@kubectl logs -n nats-dev -l app.kubernetes.io/name=nats-cluster --tail=10

# Show logs
dev-logs:
	@kubectl logs -n nats-dev -l app.kubernetes.io/name=nats-cluster -f

# Test NATS connection (requires nats CLI)
dev-test:
	@echo "🧪 Testing NATS connection..."
	@echo "Make sure port forwarding is running in another terminal"
	@echo "Testing publish..."
	@nats pub test.dev "Hello from dev setup!" --server nats://localhost:4222 || echo "Install nats CLI: https://github.com/nats-io/natscli"

# Purge all NATS streams
nats-purge-streams:
	@echo "Purging all NATS streams..."
	@echo "Make sure port forwarding is running in another terminal"
	@nats stream purge --all --force --server nats://localhost:4222 2>/dev/null || true

# Complete development setup
dev-setup: k8s-setup dev-deploy
	@echo "✅ Complete development setup complete!"
	@echo "Run 'make dev-port-forward' in another terminal to access NATS"

# Restart development deployment
dev-restart: dev-clean dev-deploy

help:
	@echo "NATS Development Commands:"
	@echo "  make k8s-setup        - Set up local Kubernetes cluster"
	@echo "  make k8s-clean        - Clean up Kubernetes cluster"
	@echo "  make dev-deploy       - Deploy NATS for development"
	@echo "  make dev-clean        - Clean up development deployment"
	@echo "  make dev-port-forward - Set up port forwarding"
	@echo "  make dev-status       - Check deployment status"
	@echo "  make dev-logs         - Show live logs"
	@echo "  make dev-test         - Test NATS connection"
	@echo "  make dev-setup        - Complete setup (K8s + NATS)"
	@echo "  make dev-restart      - Clean and redeploy"
	@echo "  make nats-purge-streams - Purge all NATS streams"
