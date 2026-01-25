#!/bin/bash
# Backend Discovery Tool
# Usage: ./discover.sh [port]
# Default port: 8889 (MCP HTTP)

set -e

PORT=${1:-8889}
WS_PORT=$((PORT-1))

echo "═══════════════════════════════════════════"
echo "  Backend Discovery Tool"
echo "═══════════════════════════════════════════"
echo ""

# Try MCP HTTP discovery
echo "→ Attempting MCP HTTP discovery on port $PORT..."
echo ""

response=$(curl -s -X POST "http://localhost:$PORT/mcp" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "clientInfo": {"name": "discovery-tool", "version": "1.0.0"},
      "capabilities": {}
    }
  }' 2>/dev/null || echo "")

if [ -n "$response" ] && echo "$response" | jq -e '.result.serverInfo' > /dev/null 2>&1; then
  name=$(echo "$response" | jq -r '.result.serverInfo.name')
  version=$(echo "$response" | jq -r '.result.serverInfo.version')
  has_tools=$(echo "$response" | jq -e '.result.capabilities.tools' > /dev/null 2>&1 && echo "✓" || echo "✗")
  has_logging=$(echo "$response" | jq -e '.result.capabilities.logging' > /dev/null 2>&1 && echo "✓" || echo "✗")

  echo "✓ MCP Backend Discovered!"
  echo ""
  echo "  Namespace:     $name"
  echo "  Version:       $version"
  echo "  Capabilities:"
  echo "    - Tools:     $has_tools"
  echo "    - Logging:   $has_logging"
  echo ""
  echo "  Connection Info:"
  echo "    MCP HTTP:    http://localhost:$PORT/mcp"
  echo "    WebSocket:   ws://localhost:$WS_PORT (likely)"
  echo ""
  echo "  How to call methods:"
  echo "    MCP:         Use MCP client libraries"
  echo "    WebSocket:   ${name}.call {\"method\": \"echo.echo\", \"params\": {...}}"
  echo ""
  echo "  View full schema:"
  echo "    curl -X POST http://localhost:$PORT/mcp \\"
  echo "      -H 'Content-Type: application/json' \\"
  echo "      -d '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}'"
  echo ""

  # Show available tools if it's a hub
  echo "→ Discovering available tools..."
  echo ""

  tools_response=$(curl -s -X POST "http://localhost:$PORT/mcp" \
    -H "Content-Type: application/json" \
    -d '{
      "jsonrpc": "2.0",
      "id": 2,
      "method": "tools/list"
    }' 2>/dev/null || echo "")

  if [ -n "$tools_response" ] && echo "$tools_response" | jq -e '.result.tools' > /dev/null 2>&1; then
    tool_count=$(echo "$tools_response" | jq -r '.result.tools | length')
    echo "  Available Tools: $tool_count"
    echo ""
    echo "$tools_response" | jq -r '.result.tools[] | "    • \(.name) - \(.description // "No description")"' | head -20

    if [ "$tool_count" -gt 20 ]; then
      echo "    ... (showing first 20 of $tool_count tools)"
    fi
  fi

  echo ""
  echo "═══════════════════════════════════════════"
  echo "  Discovery Complete ✓"
  echo "═══════════════════════════════════════════"

  exit 0
else
  echo "✗ MCP HTTP not available on port $PORT"
  echo ""
  echo "  Try:"
  echo "    - WebSocket discovery on port $WS_PORT"
  echo "    - Different port number"
  echo "    - Start the backend server"
  echo ""
  echo "  Common namespaces to try:"
  echo "    - substrate"
  echo "    - hub"
  echo "    - plexus"
  echo ""

  exit 1
fi
