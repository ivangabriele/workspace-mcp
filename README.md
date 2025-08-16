# Workspace MCP

**MCP-ize your workspace right from your IDE!**

MCP server with IDEs extensions to CRUD current workspace files and run CLI commands within it,
exposed via [`cloudflared`](https://github.com/cloudflare/cloudflared) to 

## First Release Scope

- [ ] VSCode extension
  - [ ] MCP Server Start (via `cloudflared`
  - [ ] MCP Server Stop
  - [ ] MCP Server Request and Response logging (Output panel, listening to MCP Server Telemetry Endpoint)
  - [ ] MCP Server Credentials management (via settings)
- [ ] MCP Server
  - **MCP Server Tools:**
    - [ ] Read main (= root) `AGENTS.md` file
    - [ ] CRUD current workspace files
      - [ ] List files (workspace relative paths, recursive or not with customizable depth)
      - [ ] Read file content (entire file or partial read from line number to line number)
      - [ ] Create file
      - [ ] Update file (entire file or partial update from line number to line number)
      - [ ] Delete file
    - [ ] Run CLI commands within current workspace
  - **Telemetry Endpoint (separate JSON-RPC port):**
    - [ ] Broadcast MCP Server requests and responses
