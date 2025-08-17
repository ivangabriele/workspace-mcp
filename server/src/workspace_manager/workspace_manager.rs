use std::path::PathBuf;

use rmcp::{
    ErrorData, Json, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{self, ListPromptsResult, ListResourceTemplatesResult, PaginatedRequestParam},
    service::RequestContext,
    tool, tool_handler, tool_router,
};

use crate::workspace_manager::types::{ListFilesRequest, ListFilesResponse};

#[derive(Clone)]
pub struct WorkspaceManager {
    tool_router: ToolRouter<WorkspaceManager>,
    workspace_path: PathBuf,
}
#[tool_router]
impl WorkspaceManager {
    pub fn new(workspace_path_as_string: String) -> Self {
        let workspace_path = PathBuf::from(workspace_path_as_string);

        Self {
            tool_router: Self::tool_router(),
            workspace_path,
        }
    }

    #[tool(description = "List files in the current workspace directory.")]
    pub fn list_files(
        &self,
        Parameters(ListFilesRequest { path }): Parameters<ListFilesRequest>,
    ) -> Result<Json<ListFilesResponse>, String> {
        let path: String = path.unwrap_or_else(|| ".".to_string());
        let full_path = self.workspace_path.join(path);
        let entries = std::fs::read_dir(full_path).unwrap();
        let files: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().into_string().unwrap_or_default())
            .collect();

        Ok(Json(ListFilesResponse { files }))
    }
}

#[tool_handler]
impl ServerHandler for WorkspaceManager {
    fn get_info(&self) -> model::ServerInfo {
        model::ServerInfo {
            protocol_version: model::ProtocolVersion::V_2024_11_05,
            capabilities: model::ServerCapabilities::builder()
                .enable_prompts()
                .enable_resources()
                .enable_tools()
                .build(),
            server_info: model::Implementation::from_build_env(),
            instructions: Some("This MCP server provides tools to CRUD files and run CLI commands within the user-defined workspace.".to_string()),
        }
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, ErrorData> {
        Ok(ListPromptsResult {
            next_cursor: None,
            prompts: Vec::new(),
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        Ok(ListResourceTemplatesResult {
            next_cursor: None,
            resource_templates: Vec::new(),
        })
    }
}
