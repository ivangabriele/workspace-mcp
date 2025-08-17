use rmcp::schemars;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListFilesRequest {
    #[schemars(description = "Workspace relative path to list files from.")]
    pub path: Option<String>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ListFilesResponse {
    pub files: Vec<String>,
}
