use std::path::PathBuf;

use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model, tool, tool_handler, tool_router,
};

use crate::file_manager::types::{ListFilesRequest, ListFilesResponse};

#[derive(Clone)]
pub struct FileManager {
    tool_router: ToolRouter<FileManager>,
    workspace_path: PathBuf,
}

#[tool_router]
impl FileManager {
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
    ) -> Json<ListFilesResponse> {
        let path: String = path.unwrap_or_else(|| ".".to_string());
        let full_path = self.workspace_path.join(path);
        let entries = std::fs::read_dir(full_path).unwrap();
        let files: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().into_string().unwrap_or_default())
            .collect();

        Json(ListFilesResponse { files })
    }
}

#[tool_handler]
impl ServerHandler for FileManager {
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
}
