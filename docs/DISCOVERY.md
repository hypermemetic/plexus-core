# Backend Discovery Guide

How to discover backend information when you only have port/connection info.

## Problem

You have a backend running on a port (e.g., `localhost:8888`) but don't know:
- What namespace to use for calls
- What methods are available
- What version is running

## Discovery Methods by Transport

### MCP HTTP (Recommended)

**Port**: Default 8889 (one port above WebSocket)

The MCP initialize handshake automatically provides server info:

```bash
# Using MCP Inspector
npx @modelcontextprotocol/inspector http://localhost:8889/mcp

# Or manually with curl
curl -X POST http://localhost:8889/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "clientInfo": {"name": "discovery", "version": "1.0.0"},
      "capabilities": {}
    }
  }'

# Response includes:
{
  "result": {
    "serverInfo": {
      "name": "substrate",      # ← Namespace/backend name
      "version": "0.2.6"        # ← Version
    },
    "capabilities": {
      "tools": {},              # Has methods
      "logging": {}             # Supports logging
    }
  }
}
```

### JSON-RPC WebSocket

**Port**: Default 8888

Call the `.schema` method to discover the backend:

```bash
# Using wscat
wscat -c ws://localhost:8888

# Send discovery request (works for any namespace)
> {"jsonrpc":"2.0","id":1,"method":"substrate.schema_subscribe"}

# For unknown namespace, try common defaults:
> {"jsonrpc":"2.0","id":1,"method":"hub.schema_subscribe"}
> {"jsonrpc":"2.0","id":1,"method":"plexus.schema_subscribe"}

# Response:
{
  "jsonrpc": "2.0",
  "result": {
    "namespace": "substrate",
    "version": "0.2.6",
    "description": "Substrate activation server",
    "methods": [
      {"name": "call", "description": "..."},
      {"name": "hash", "description": "..."},
      {"name": "schema", "description": "..."}
    ],
    "children": [
      {"namespace": "echo", "description": "..."},
      {"namespace": "arbor", "description": "..."}
    ]
  }
}
```

### JSON-RPC Stdio

If you have access to stdin/stdout of the process:

```bash
# Send discovery request
echo '{"jsonrpc":"2.0","id":1,"method":"substrate.schema_subscribe"}' | ./backend

# Response on stdout
{"jsonrpc":"2.0","result":{"namespace":"substrate",...}}
```

## Discovery Strategy

### 1. **Try MCP HTTP First** (Easiest)

MCP HTTP is designed for discovery - the initialize handshake is required and always returns server info.

```bash
# Check if MCP is running (typically port+1 from WebSocket)
curl http://localhost:8889/mcp 2>/dev/null && echo "MCP available"
```

### 2. **Try Common Namespaces** (WebSocket fallback)

If MCP isn't available, try these common namespace patterns:

```javascript
const commonNamespaces = [
  'substrate',  // Substrate server
  'hub',        // Generic DynamicHub
  'plexus',     // Legacy name
  'app',        // Generic application
];

for (const ns of commonNamespaces) {
  try {
    await ws.send({
      jsonrpc: '2.0',
      id: 1,
      method: `${ns}.schema_subscribe`
    });
    // If successful, this is the namespace
    break;
  } catch (e) {
    continue;
  }
}
```

### 3. **Use Port Conventions** (Convention-based)

Standard substrate port layout:
- `4444`: WebSocket JSON-RPC
- `4445`: MCP HTTP

## Example: Full Discovery Flow

```bash
#!/bin/bash
# Discover backend on unknown port

PORT=${1:-8889}  # Default to MCP port

echo "Attempting MCP discovery on port $PORT..."

# Try MCP initialize
response=$(curl -s -X POST "http://localhost:$PORT/mcp" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "clientInfo": {"name": "discovery", "version": "1.0.0"},
      "capabilities": {}
    }
  }')

if echo "$response" | jq -e '.result.serverInfo' > /dev/null 2>&1; then
  name=$(echo "$response" | jq -r '.result.serverInfo.name')
  version=$(echo "$response" | jq -r '.result.serverInfo.version')
  echo "✓ Found: $name v$version"
  echo "  Namespace: $name"
  echo "  Version: $version"
  echo "  MCP URL: http://localhost:$PORT/mcp"
else
  echo "✗ MCP discovery failed, try WebSocket on port $((PORT-1))"
fi
```

## What Gets Returned

### PluginSchema Structure

```json
{
  "namespace": "substrate",
  "version": "0.2.6",
  "description": "Short description (max 15 words)",
  "long_description": "Detailed description...",
  "hash": "abc123...",  // Configuration hash
  "methods": [
    {
      "name": "call",
      "description": "Route a call to a registered activation",
      "hash": "method-hash",
      "params": { /* JSON Schema */ }
    }
  ],
  "children": [  // For hubs like DynamicHub or Solar
    {
      "namespace": "echo",
      "description": "Echo messages back",
      "hash": "child-hash"
    }
  ]
}
```

### For DynamicHub

DynamicHub returns a **hub schema** with children:

```json
{
  "namespace": "substrate",  // The DynamicHub's namespace
  "version": "0.2.6",
  "description": "Central routing and introspection",
  "methods": ["call", "hash", "schema"],
  "children": [
    {"namespace": "echo", ...},
    {"namespace": "arbor", ...},
    {"namespace": "cone", ...}
  ]
}
```

From this you know:
- Main calls: `substrate.call`, `substrate.hash`
- Child calls: `echo.echo`, `arbor.tree_create`, etc.

### For Single Activations

Single activations return a **leaf schema**:

```json
{
  "namespace": "echo",
  "version": "1.0.0",
  "description": "Echo messages back",
  "methods": ["echo", "schema"],
  "children": null  // Not a hub
}
```

## Integration Examples

### TypeScript Client

```typescript
async function discoverBackend(baseUrl: string): Promise<BackendInfo> {
  // Try MCP first
  const mcpUrl = `${baseUrl}/mcp`;

  try {
    const response = await fetch(mcpUrl, {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'initialize',
        params: {
          protocolVersion: '2024-11-05',
          clientInfo: {name: 'client', version: '1.0.0'},
          capabilities: {}
        }
      })
    });

    const data = await response.json();
    const info = data.result.serverInfo;

    return {
      namespace: info.name,
      version: info.version,
      transport: 'mcp',
      endpoint: mcpUrl
    };
  } catch (e) {
    throw new Error('MCP discovery failed - try WebSocket');
  }
}
```

### Python Client

```python
import requests

def discover_backend(port=8889):
    """Discover backend via MCP initialize."""
    url = f'http://localhost:{port}/mcp'

    payload = {
        'jsonrpc': '2.0',
        'id': 1,
        'method': 'initialize',
        'params': {
            'protocolVersion': '2024-11-05',
            'clientInfo': {'name': 'discovery', 'version': '1.0.0'},
            'capabilities': {}
        }
    }

    response = requests.post(url, json=payload)
    result = response.json()['result']

    return {
        'namespace': result['serverInfo']['name'],
        'version': result['serverInfo']['version'],
        'capabilities': result['capabilities']
    }

# Usage
backend = discover_backend(8889)
print(f"Found: {backend['namespace']} v{backend['version']}")
```

## Best Practices

1. **Always try MCP first** - It's designed for discovery with required handshake
2. **Cache discovered info** - Namespace and version rarely change during runtime
3. **Use port conventions** - WebSocket on N, MCP on N+1
4. **Fallback gracefully** - Try common namespaces if discovery fails
5. **Check hash for changes** - The `.hash` method detects configuration updates

## Future Improvements

Potential additions for better discovery:

1. **HTTP GET /info endpoint** - Simple HTTP endpoint returning JSON
2. **Well-known port registry** - Standard port assignments for services
3. **mDNS/DNS-SD** - Network-wide service discovery
4. **Health check with identity** - `/health` endpoint with namespace/version

## Related Documentation

- **MCP Protocol**: See rmcp documentation for full MCP spec
- **Activation Trait**: `src/plexus/plexus.rs` - namespace(), version(), description()
- **PluginSchema**: `src/plexus/schema.rs` - Schema types and structure
