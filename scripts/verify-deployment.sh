#!/bin/bash

# Verification script for Crust Discord Bot Operator deployment
# This script checks that all required files and tools are in place

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "🔍 Bedrock Discord Bot Operator - Deployment Verification"
echo "======================================================"

# Check tools
echo
echo "📋 Checking Prerequisites..."

check_tool() {
    if command -v "$1" &> /dev/null; then
        echo -e "${GREEN}✓${NC} $1 is installed"
        return 0
    else
        echo -e "${RED}✗${NC} $1 is not installed"
        return 1
    fi
}

tools_ok=true
check_tool "docker" || tools_ok=false
check_tool "kind" || tools_ok=false
check_tool "kubectl" || tools_ok=false

if [ "$tools_ok" = false ]; then
    echo -e "${RED}Missing required tools. Please install them and try again.${NC}"
    exit 1
fi

# Check Docker permissions
echo
echo "🐳 Checking Docker permissions..."
if docker version &> /dev/null; then
    echo -e "${GREEN}✓${NC} Docker is accessible"
else
    echo -e "${YELLOW}⚠${NC} Docker permission issue detected"
    echo "  Run: sudo usermod -aG docker \$USER && newgrp docker"
fi

# Check files
echo
echo "📁 Checking Required Files..."

check_file() {
    if [ -f "$1" ]; then
        echo -e "${GREEN}✓${NC} $1"
        return 0
    else
        echo -e "${RED}✗${NC} $1 (missing)"
        return 1
    fi
}

files_ok=true

# Deployment script
check_file "scripts/deploy-kind.sh" || files_ok=false
check_file "scripts/shutdown-deployment.sh" || files_ok=false

# Kubernetes manifests
check_file "k8s/nats-cluster.yaml" || files_ok=false
check_file "k8s/operator-deployment.yaml" || files_ok=false
check_file "k8s/rbac.yaml" || files_ok=false
check_file "crd/shardcluster-crd.yaml" || files_ok=false

# Dockerfiles
check_file "Dockerfile.crust" || files_ok=false
check_file "Dockerfile.stratum" || files_ok=false

# Rust source files
check_file "bot/crates/crust/src/main.rs" || files_ok=false
check_file "bot/crates/crust/src/lib.rs" || files_ok=false
check_file "bot/crates/stratum/src/main.rs" || files_ok=false

# Documentation
check_file "DEPLOYMENT.md" || files_ok=false
check_file "README-DEPLOYMENT-SUMMARY.md" || files_ok=false

# Check if deployment script is executable
echo
echo "🔧 Checking Permissions..."
if [ -x "scripts/deploy-kind.sh" ]; then
    echo -e "${GREEN}✓${NC} Deployment script is executable"
else
    echo -e "${YELLOW}⚠${NC} Making deployment script executable..."
    chmod +x scripts/deploy-kind.sh
    echo -e "${GREEN}✓${NC} Fixed deployment script permissions"
fi

# Test Rust build
echo
echo "🦀 Testing Rust Build..."
cd bot
if cargo check --quiet 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Rust project compiles successfully"
else
    echo -e "${RED}✗${NC} Rust build failed"
    echo "  Try: cd bot && cargo build"
    files_ok=false
fi
cd ..

# Summary
echo
echo "📊 Verification Summary"
echo "======================="

if [ "$files_ok" = true ]; then
    echo -e "${GREEN}✅ All checks passed!${NC}"
    echo
    echo "🚀 Ready to deploy! Run:"
    echo "  ./scripts/deploy-kind.sh --token YOUR_DISCORD_BOT_TOKEN"
    echo
    echo "📖 For detailed instructions, see:"
    echo "  - DEPLOYMENT.md"
    echo "  - README-DEPLOYMENT-SUMMARY.md"
else
    echo -e "${RED}❌ Some checks failed.${NC}"
    echo "Please fix the issues above and run this script again."
    exit 1
fi
