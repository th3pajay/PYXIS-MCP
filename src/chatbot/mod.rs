use std::{sync::{Arc, Mutex}, time::Duration};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{pyxis_client::PyxisClient, tools, vector_search::VectorIndex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: "system".into(), content: Some(content.into()), tool_calls: None, tool_call_id: None }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: "user".into(), content: Some(content.into()), tool_calls: None, tool_call_id: None }
    }
    pub fn tool_result(id: String, content: String) -> Self {
        Self { role: "tool".into(), content: Some(content), tool_calls: None, tool_call_id: Some(id) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub reply: String,
    pub tool_calls: Vec<(String, String)>,
}

pub struct Chatbot {
    pyxis: Arc<PyxisClient>,
    index: Arc<Mutex<VectorIndex>>,
    base_url: String,
    model: String,
    http: reqwest::Client,
}

impl Chatbot {
    pub fn new(
        pyxis: Arc<PyxisClient>,
        index: Arc<Mutex<VectorIndex>>,
        base_url: String,
        model: String,
    ) -> Self {
        Self {
            pyxis, index, base_url, model,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(90))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn chat(&self, history: Vec<Message>, user_msg: &str) -> Result<ChatResponse> {
        let mut messages = vec![Message::system(SYSTEM_PROMPT)];
        messages.extend(history);
        messages.push(Message::user(user_msg));

        let mut tool_call_log: Vec<(String, String)> = Vec::new();

        for _ in 0..8 {
            let body = serde_json::json!({
                "model": self.model,
                "messages": messages,
                "tools": tools_schema(),
                "stream": false,
                "max_tokens": 2048,
            });

            let http_resp = self.http
                .post(format!("{}/v1/chat/completions", self.base_url))
                .json(&body)
                .send().await
                .map_err(|e| { eprintln!("[chatbot] send error: {e}"); e })?;

            let status = http_resp.status();
            let raw_text = http_resp.text().await
                .map_err(|e| { eprintln!("[chatbot] body read error: {e}"); e })?;
            eprintln!("[chatbot] HTTP {status} raw response: {raw_text}");

            let resp: Value = serde_json::from_str(&raw_text)
                .map_err(|e| { eprintln!("[chatbot] JSON parse error: {e}"); anyhow::anyhow!(e) })?;

            let choice = &resp["choices"][0]["message"];
            let finish_reason = resp["choices"][0]["finish_reason"].as_str().unwrap_or("");

            let raw_calls: Vec<ToolCall> = serde_json::from_value(
                choice["tool_calls"].clone()
            ).unwrap_or_default();

            eprintln!("[chatbot] finish_reason={finish_reason:?} tool_calls={} content={:?}",
                raw_calls.len(),
                choice["content"].as_str());

            if raw_calls.is_empty() {
                let reply = choice["content"].as_str().unwrap_or("").to_string();
                eprintln!("[chatbot] returning reply (len={})", reply.len());
                return Ok(ChatResponse { reply, tool_calls: tool_call_log });
            }

            eprintln!("[chatbot] dispatching: {:?}",
                raw_calls.iter().map(|c| &c.function.name).collect::<Vec<_>>());

            messages.push(Message {
                role: "assistant".into(),
                content: None,
                tool_calls: Some(raw_calls.clone()),
                tool_call_id: None,
            });

            for call in &raw_calls {
                let args: Value = serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
                let result = self.dispatch(&call.function.name, args).await;
                tool_call_log.push((call.function.name.clone(), result.clone()));
                messages.push(Message::tool_result(call.id.clone(), result));
            }
        }

        Ok(ChatResponse {
            reply: "Reached tool call limit without a final answer.".into(),
            tool_calls: tool_call_log,
        })
    }

    async fn dispatch(&self, tool: &str, args: Value) -> String {
        match tool {
            "list_skus"       => tools::list_skus(&self.pyxis).await,
            "list_inventory"  => tools::list_inventory(&self.pyxis).await,
            "search_skus"     => tools::search_skus(self.index.clone(), &args).await,
            "search_bins"     => tools::search_bins(self.index.clone(), &args).await,
            "check_inventory" => tools::check_inventory(&self.pyxis, &args).await,
            "propose_move"    => tools::propose_move(&self.pyxis, &args).await,
            _ => serde_json::json!({"error": format!("unknown tool: {tool}")}).to_string(),
        }
    }
}

fn tools_schema() -> Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "list_skus",
                "description": "List all SKUs in the warehouse catalogue. Use this to answer questions about how many SKUs exist, what SKUs are available, or to get all SKU IDs before checking inventory.",
                "parameters": {"type": "object", "properties": {}}
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_inventory",
                "description": "Return live inventory for every SKU in one call: on_hand, available, committed, on_order, and bin locations. Use for overall summaries or shortage reports.",
                "parameters": {"type": "object", "properties": {}}
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_skus",
                "description": "Search SKUs by natural language description (e.g. 'frozen food', 'heavy machinery'). Returns ranked SKU IDs.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "limit": {"type": "integer", "default": 5}
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_bins",
                "description": "Search warehouse bins by natural language query. Returns ranked candidate bins.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"},
                        "limit": {"type": "integer", "default": 5}
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "check_inventory",
                "description": "Check live inventory quantity for a SKU.",
                "parameters": {
                    "type": "object",
                    "properties": {"sku_id": {"type": "string"}},
                    "required": ["sku_id"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "propose_move",
                "description": "Propose a bin move. Runs tiered safety validation then stages the move.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "sku_id": {"type": "string"},
                        "from_bin_id": {"type": "string"},
                        "to_bin_id": {"type": "string"},
                        "user_id": {"type": "string"}
                    },
                    "required": ["sku_id", "from_bin_id", "to_bin_id", "user_id"]
                }
            }
        }
    ])
}

const SYSTEM_PROMPT: &str = "You are PYXIS, an AI warehouse management investigator. \
You have live tools — always use them before answering. \
Tool selection rules: \
- Catalogue questions (\"how many SKUs\", \"what SKUs exist\") → call list_skus. \
- Inventory summary or shortage report → call list_inventory. \
- Single SKU inventory → call check_inventory with the sku_id. \
- Vague SKU description (\"frozen food\", \"heavy pallet\") → call search_skus first. \
- Vague bin location (\"cold area\", \"near aisle A\") → call search_bins first. \
- Move request → search_skus → check_inventory → search_bins → propose_move. \
Never say you lack a tool without first trying the most relevant one. \
Chain calls within a turn until you have a complete answer. \
Be concise in final replies.";
