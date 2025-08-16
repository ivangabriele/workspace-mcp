set dotenv-load
# set positional-arguments

default:
  just --list

[working-directory: 'server']
build-server:
  cargo build --release

[working-directory: '.']
expose:
  cloudflared tunnel run --token "${CLOUDFLARED_TOKEN}"

[working-directory: '.']
install:
  bun install

# Serve the MCP server using the justfile directory path as the `--workspace` parameter.
[working-directory: 'server']
serve:
  cargo run --release -- --auth-token "${WORKSPACE_MCP_AUTH_TOKEN}" --workspace-path "{{ justfile_directory() }}"

# Check the MCP server by sending a JSON-RPC request to list files in the workspace root directory.
[working-directory: '.']
check-local:
  curl -N "http://0.0.0.0:9876/sse" \
    -H "Authorization: Bearer ${WORKSPACE_MCP_AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","id":1,"method":"list_files","params":{"path":"."}}'

[working-directory: '.']
check-public:
  curl -N "https://${CLOUDFLARED_TUNNEL_DOMAIN}/sse" \
    -H "Authorization: Bearer ${WORKSPACE_MCP_AUTH_TOKEN}" \
    -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","id":1,"method":"list_files","params":{"path":"."}}'
[working-directory: '.']
check-public-fail:
  curl -N "https://${CLOUDFLARED_TUNNEL_DOMAIN}/sse" \
    -H "Authorization: Bearer WRONG_TOKEN" \
    -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","id":1,"method":"list_files","params":{"path":"."}}'
