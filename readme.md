# Cedar Authorization Agent

A lightweight HTTP service that evaluates Cedar authorization policies for the PL0 Product Service.

## Purpose

This service provides policy-based access control (PBAC) for our application using AWS Cedar Policy Language. It evaluates authorization requests against defined policies and returns Allow/Deny decisions.

## Architecture

- **Input**: Authorization request (principal, action, resource, entities)
- **Processing**: Evaluates against Cedar policies
- **Output**: Decision (Allow/Deny) with diagnostic information

## What's Inside

```
cedar-agent-custom/
├── src/
│   └── main.rs          # HTTP server with Cedar policy evaluation
├── Cargo.toml           # Rust dependencies
├── Dockerfile           # Container build instructions
├── README.md            # This file
└── DEPLOYMENT.md        # Deployment guide
```

## Quick Start

### Prerequisites
- Docker installed
- Cedar policies (from main project)

### Build the Image

```bash
# Clone this repository
git clone git@github.com:kapilmpradhan/Cedar-agent.git
cd cedar-custom

# Build Docker image
docker build -t pl0-cedar-agent:latest .

# Verify build
docker images | grep cedar-agent
```

### Run Locally

```bash
# Run with policies mounted from your main project
docker run --rm -p 8181:8181 \
  -v /path/to/pl0-backend-shared/src/cedar-authz/policies:/app/policies:ro \
  pl0-cedar-agent:latest

# Test health endpoint
curl http://localhost:8181/health
# Expected: {"status":"ok"}

```

### Test Authorization

```bash
# Test same-branch access (should Allow)
curl -X POST http://localhost:8181/authorize \
  -H "Content-Type: application/json" \
  -d '{
    "principal": "Member::\"13\"",
    "action": "Action::\"CreateProduct\"",
    "resource": "Branch::\"1\"",
    "entities": [
      {
        "uid": {"__entity": {"type": "Member", "id": "13"}},
        "attrs": {"role": "owner", "branchId": 1},
        "parents": []
      },
      {
        "uid": {"__entity": {"type": "Branch", "id": "1"}},
        "attrs": {"id": 1, "userId": 1, "name": "Test Branch"},
        "parents": []
      }
    ]
  }'

# Expected: {"decision":"Allow","diagnostics":{"reason":["staff-manage-branch-products"],"errors":[]}}

# Test cross-branch access (should Deny)
curl -X POST http://localhost:8181/authorize \
  -H "Content-Type: application/json" \
  -d '{
    "principal": "Member::\"13\"",
    "action": "Action::\"CreateProduct\"",
    "resource": "Branch::\"8\"",
    "entities": [
      {
        "uid": {"__entity": {"type": "Member", "id": "13"}},
        "attrs": {"role": "owner", "branchId": 1},
        "parents": []
      },
      {
        "uid": {"__entity": {"type": "Branch", "id": "8"}},
        "attrs": {"id": 8, "userId": 2, "name": "Another Branch"},
        "parents": []
      }
    ]
  }'

# Expected: {"decision":"Deny","diagnostics":{"reason":["deny-cross-branch-branch"],"errors":[]}}
```

## 🔧 Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CEDAR_POLICY_PATH` | `/app/policies/policy.cedar` | Path to Cedar policy file |
| `CEDAR_SCHEMA_PATH` | `/app/policies/schema.cedarschema.json` | Path to Cedar schema file |
| `BIND_ADDR` | `0.0.0.0:8181` | Server bind address |
| `RUST_LOG` | `info` | Log level (debug, info, warn, error) |

### Docker Compose Example

```yaml
services:
  cedar-agent:
    image: pl0-cedar-agent:latest
    ports:
      - "8181:8181"
    volumes:
      - ./src/cedar-authz/policies:/app/policies:ro
    environment:
      - RUST_LOG=info
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8181/health"]
      interval: 30s
      timeout: 10s
      retries: 3
```

## API Reference

### Health Check

```http
GET /health
```

**Response:**
```json
{
  "status": "healthy"
}
```

### Authorization

```http
POST /authorize
Content-Type: application/json
```

**Request Body:**
```json
{
  "principal": "Member::\"<member-id>\"",
  "action": "Action::\"<action-name>\"",
  "resource": "<ResourceType>::\"<resource-id>\"",
  "entities": [
    {
      "uid": {
        "__entity": {
          "type": "Member",
          "id": "<member-id>"
        }
      },
      "attrs": {
        "role": "owner",
        "branchId": 1
      },
      "parents": []
    }
  ]
}
```

**Response:**
```json
{
  "decision": "Allow",
  "diagnostics": {
    "reason": ["policy-id-that-allowed"],
    "errors": []
  }
}
```

## Cedar Policies

Cedar policies are maintained in the main project at:
```
pl0-backend-shared/src/cedar-authz/policies/
├── policy.cedar           # Authorization policies
└── schema.cedarschema.json  # Entity and action schema
```

See the main project's documentation for policy details.

## Development

### Making Changes

1. Edit `src/main.rs`
2. Rebuild Docker image: `docker build -t pl0-cedar-agent:test .`
3. Test locally with your policies
4. If tests pass, tag and push: `docker tag pl0-cedar-agent:test pl0-cedar-agent:v1.1.0`

### Dependencies

- **cedar-policy** (v4.2): Core Cedar policy evaluation engine
- **tokio**: Async runtime
- **hyper**: HTTP server
- **serde/serde_json**: JSON serialization

To update dependencies, edit `Cargo.toml` and rebuild.

## Deployment

### Quick Deployment Steps

1. **Build image:**
   ```bash
   docker build -t pl0-cedar-agent:v1.0.0 .
   ```

2. **Create tar file for distribution:**
   ```bash
   docker save pl0-cedar-agent:v1.0.0 -o cedar-agent-v1.0.0.tar
   ```

3. **Load on target system:**
   ```bash
   docker load -i cedar-agent-v1.0.0.tar
   ```

4. **Deploy with docker-compose:**
   ```bash
   docker-compose up -d cedar-agent
   ```

## Troubleshooting

### Container exits immediately
```bash
# Check logs
docker logs <container-id>
```

### Port already in use
```bash
# Find conflicting container
docker ps | grep 8181

# Stop it
docker stop <container-id>
```

### Policy evaluation errors
```bash
# Check policy syntax
cat /path/to/policy.cedar

# Validate schema
cat /path/to/schema.cedarschema.json | jq .
```

## Monitoring

### Health Checks
```bash
# Basic health check
curl http://localhost:8181/health

# From Kubernetes liveness probe
curl -f http://cedar-agent:8181/health || exit 1
```

### Logs
```bash
# Container logs
docker logs -f cedar-agent

# Filter for authorization decisions
docker logs cedar-agent | grep "Authorization decision"
```

## Contributing

1. Create a feature branch
2. Make your changes
3. Test locally
4. Submit PR with description of changes

## License

Internal use only - True Serve

## Related Projects

- **pl0-backend-shared**: Contains Cedar policies and integration code