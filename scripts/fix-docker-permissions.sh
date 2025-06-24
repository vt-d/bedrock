#!/bin/bash

echo "🔧 Bedrock Docker Permissions Setup"
echo "====================================="

# Check if user is in docker group
if groups $USER | grep -q '\bdocker\b'; then
    echo "✅ User $USER is already in the docker group"
else
    echo "❌ User $USER is not in the docker group"
    echo ""
    echo "To fix this, run these commands:"
    echo "1. Add user to docker group:"
    echo "   sudo usermod -aG docker \$USER"
    echo ""
    echo "2. Apply the group change:"
    echo "   newgrp docker"
    echo ""
    echo "3. Or log out and log back in"
    echo ""
    echo "4. Test Docker access:"
    echo "   docker version"
    exit 1
fi

# Test Docker access
echo ""
echo "Testing Docker access..."
if docker version &>/dev/null; then
    echo "✅ Docker is accessible"
else
    echo "❌ Docker is not accessible"
    echo ""
    echo "Try running: newgrp docker"
    echo "Or log out and log back in"
    exit 1
fi

# Test Kind
echo ""
echo "Testing Kind..."
if kind version &>/dev/null; then
    echo "✅ Kind is available"
    
    # Show existing clusters
    echo ""
    echo "Existing Kind clusters:"
    kind get clusters 2>/dev/null || echo "No clusters found"
else
    echo "❌ Kind is not available"
    exit 1
fi

echo ""
echo "🎉 Everything looks good! You can now run the deployment script."
