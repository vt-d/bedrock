# Production Configuration Values

## NATS Cluster
- **Replicas**: 3 (High availability)
- **CPU**: 500m request, 1000m limit
- **Memory**: 512Mi request, 1Gi limit
- **Storage**: 10Gi per pod
- **JetStream**: 512MB memory store, 10GB file store

## Crust Operator
- **Replicas**: 2 (High availability with leader election)
- **Image**: bedrock-operator:v1.0.0 (Tagged version)
- **CPU**: 200m request, 500m limit
- **Memory**: 256Mi request, 512Mi limit
- **Logging**: info level (reduced verbosity)
- **Leader Election**: Enabled

## Twilight Gateway Proxy
- **Replicas**: 3 (Load distribution)
- **CPU**: 500m request, 1000m limit
- **Memory**: 512Mi request, 2Gi limit
- **Health Checks**: Liveness and readiness probes
- **Timeouts**: 30 second proxy timeout
- **Connections**: 1000 max connections

## Security
- **RBAC**: Added leader election permissions
- **Secrets**: Discord token stored in Kubernetes secrets
- **Security Context**: Non-root user, read-only filesystem
- **Image Pull Policy**: Always for latest security updates

## Monitoring
- **Health Checks**: HTTP health and readiness endpoints
- **Logging**: Structured logging with appropriate levels
- **Resource Limits**: Proper resource constraints

## High Availability
- **Multiple Replicas**: All components have multiple replicas
- **Leader Election**: Enabled for operator
- **Load Balancing**: Services distribute traffic
- **Persistent Storage**: NATS data persistence
