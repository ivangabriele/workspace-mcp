# Workspace MCP

**MCP-ize your workspace right from your IDE!**

MCP server with IDEs extensions to CRUD current workspace files and run CLI commands within it, exposed via
[`cloudflared`](https://github.com/cloudflare/cloudflared), allowing LLM agents to interact with the workspace
(ChatGPT, Claude, etc).

## First Release Scope

- [ ] VSCode extension
  - [ ] MCP Server Start (via `cloudflared`
  - [ ] MCP Server Stop
  - [ ] MCP Server Request and Response logging (Output panel)
  - [ ] MCP Server Credentials management (via settings)
- [ ] MCP Server
  - [ ] Basic Bearer authentication
  - [ ] **MCP Tools:** _(all within currently opened workspace)_
    - [ ] Read main (= root) `AGENTS.md` file
    - [ ] CRUD current workspace files
      - [ ] List files (workspace relative paths, recursive or not with customizable depth)
      - [ ] Read file content (entire file or partial read from line number to line number)
      - [ ] Create file
      - [ ] Update file (entire file or partial update from line number to line number)
      - [ ] Delete file
    - [ ] Run CLI commands
  - [ ] **IDE Endpoint:** _(separate JSON-RPC port, local)_
    - [ ] Broadcast MCP Server requests and responses
