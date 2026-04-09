use std::sync::{Arc, Mutex};

use rmcp::{
    ServerHandler, ServiceExt,
    model::{
        CallToolRequestParam, CallToolResult, Content, Implementation,
        ListToolsResult, PaginatedRequestParam, ServerInfo, Tool,
    },
    service::RequestContext,
    RoleServer,
    transport::stdio,
};

use crate::{pyxis_client::PyxisClient, tools, vector_search::VectorIndex};

#[derive(Clone)]
pub struct McpServer {
    pyxis: Arc<PyxisClient>,
    index: Arc<Mutex<VectorIndex>>,
}

impl McpServer {
    pub fn new(pyxis: Arc<PyxisClient>, index: Arc<Mutex<VectorIndex>>) -> Self {
        Self { pyxis, index }
    }

    fn tools() -> Vec<Tool> {
        vec![
            Tool::new(
                "list_skus",
                "List all SKUs in the warehouse catalogue with id, name, zone and weight.",
                serde_json::json!({"type": "object", "properties": {}}).as_object().unwrap().clone(),
            ),
            Tool::new(
                "list_inventory",
                "Return live inventory for every SKU: on_hand, available, committed, on_order, locations.",
                serde_json::json!({"type": "object", "properties": {}}).as_object().unwrap().clone(),
            ),
            Tool::new(
                "search_skus",
                "Search SKUs by natural language description (e.g. 'frozen food', 'heavy machinery'). Returns ranked SKU IDs.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "limit": {"type": "integer", "default": 5}
                    },
                    "required": ["query"]
                }).as_object().unwrap().clone(),
            ),
            Tool::new(
                "search_bins",
                "Search warehouse bins by natural language query. Returns ranked candidate bins.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "limit": {"type": "integer", "default": 5}
                    },
                    "required": ["query"]
                }).as_object().unwrap().clone(),
            ),
            Tool::new(
                "check_inventory",
                "Check live inventory quantity for a SKU.",
                serde_json::json!({
                    "type": "object",
                    "properties": {"sku_id": {"type": "string"}},
                    "required": ["sku_id"]
                }).as_object().unwrap().clone(),
            ),
            Tool::new(
                "propose_move",
                "Propose a bin move. Runs tiered safety validation then stages the move with a 10-second soft-lock.",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "sku_id": {"type": "string"},
                        "from_bin_id": {"type": "string"},
                        "to_bin_id": {"type": "string"},
                        "user_id": {"type": "string"}
                    },
                    "required": ["sku_id", "from_bin_id", "to_bin_id", "user_id"]
                }).as_object().unwrap().clone(),
            ),
        ]
    }

    async fn handle_list_skus(&self) -> String { tools::list_skus(&self.pyxis).await }
    async fn handle_list_inventory(&self) -> String { tools::list_inventory(&self.pyxis).await }
    async fn handle_search_skus(&self, args: serde_json::Value) -> String { tools::search_skus(self.index.clone(), &args).await }
    async fn handle_search_bins(&self, args: serde_json::Value) -> String { tools::search_bins(self.index.clone(), &args).await }
    async fn handle_check_inventory(&self, args: serde_json::Value) -> String { tools::check_inventory(&self.pyxis, &args).await }
    async fn handle_propose_move(&self, args: serde_json::Value) -> String { tools::propose_move(&self.pyxis, &args).await }
}

impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "pyxis-mcp".into(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: rmcp::model::ServerCapabilities {
                tools: Some(rmcp::model::ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    async fn list_tools(
        &self,
        _: PaginatedRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::Error> {
        Ok(ListToolsResult { tools: Self::tools(), next_cursor: None })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let args = serde_json::Value::Object(request.arguments.unwrap_or_default());
        let text = match request.name.as_ref() {
            "list_skus" => self.handle_list_skus().await,
            "list_inventory" => self.handle_list_inventory().await,
            "search_skus" => self.handle_search_skus(args).await,
            "search_bins" => self.handle_search_bins(args).await,
            "check_inventory" => self.handle_check_inventory(args).await,
            "propose_move" => self.handle_propose_move(args).await,
            _ => return Err(rmcp::Error::invalid_params("unknown tool", None)),
        };
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

pub async fn serve(pyxis: Arc<PyxisClient>, index: Arc<Mutex<VectorIndex>>) -> anyhow::Result<()> {
    McpServer::new(pyxis, index).serve(stdio()).await?.waiting().await?;
    Ok(())
}
